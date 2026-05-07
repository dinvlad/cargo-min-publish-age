package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"time"
)

func main() {
	port := flag.Int("port", 2015, "local port to listen on")
	minAgeDays := flag.Int("min-age-days", 14, "minimum age (in days) a crate version must be before it is selectable")
	flag.Parse()

	proxyAddr := net.JoinHostPort("localhost", fmt.Sprintf("%d", *port))

	mux := http.NewServeMux()
	mux.Handle("GET /index/{path...}", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		indexHandler(w, r, *minAgeDays)
	}))

	log.Printf("cargo-age-proxy listening on %s", proxyAddr)
	log.Fatal(http.ListenAndServe(proxyAddr, mux))
}

func indexHandler(w http.ResponseWriter, r *http.Request, minAgeDays int) {
	path := r.PathValue("path")
	statusCode := http.StatusOK

	defer func() {
		log.Printf("%s %s %d %s", r.Method, r.URL.Path, statusCode, http.StatusText(statusCode))
	}()

	resp, err := http.Get("https://index.crates.io/" + path)
	if err != nil {
		statusCode = http.StatusBadGateway
		http.Error(w, err.Error(), statusCode)
		return
	}
	defer func() {
		_ = resp.Body.Close()
	}()

	w.Header().Set("Content-Type", resp.Header.Get("Content-Type"))
	w.WriteHeader(resp.StatusCode)
	statusCode = resp.StatusCode

	if path == "config.json" {
		_, _ = io.Copy(w, resp.Body)
		return
	}

	body := bufio.NewReader(resp.Body)
	cutoff := time.Now().UTC().AddDate(0, 0, -minAgeDays)

	for {
		line, err := body.ReadBytes('\n')
		if err != nil && err != io.EOF {
			log.Printf("read error: %v", err)
			break
		}
		if len(line) == 0 {
			if err == io.EOF {
				break
			}
			continue
		}

		line = bytes.TrimSuffix(line, []byte("\n"))

		var entry map[string]any
		if e := json.Unmarshal(line, &entry); e != nil {
			log.Printf("unparseable index line: %v", e)
			if err == io.EOF {
				break
			}
			continue
		}

		if pubtimeStr, ok := entry["pubtime"].(string); ok {
			pubtime, err := time.Parse(time.RFC3339, pubtimeStr)
			if err == nil && pubtime.After(cutoff) {
				entry["yanked"] = true
				if name, ok := entry["name"].(string); ok {
					if vers, ok := entry["vers"].(string); ok {
						log.Printf("yanked %s@%s (published %s)", name, vers, pubtime.Format("2006-01-02"))
					}
				}
			}
		}

		if out, err := json.Marshal(entry); err != nil {
			statusCode = http.StatusInternalServerError
			http.Error(w, "Error marshaling upstream response", statusCode)
			break
		} else if _, err := w.Write(out); err != nil {
			break
		} else if _, err := w.Write([]byte("\n")); err != nil {
			break
		}

		if err == io.EOF {
			break
		}
	}
}
