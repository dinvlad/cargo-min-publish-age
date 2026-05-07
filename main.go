package main

import (
	"bufio"
	"encoding/json"
	"io"
	"log"
	"net"
	"net/http"
	"time"
)

var (
	proxyAddr  = net.JoinHostPort("localhost", "2015")
	minAgeDays = 14
)

func main() {
	mux := http.NewServeMux()
	mux.Handle("GET /index/{path...}", http.HandlerFunc(indexHandler))

	log.Printf("cargo-age-proxy listening on %s", proxyAddr)
	log.Fatal(http.ListenAndServe(proxyAddr, mux))
}

func indexHandler(w http.ResponseWriter, r *http.Request) {
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

	scanner := bufio.NewScanner(resp.Body)
	// Sparse index lines can be long; give scanner a 1 MiB buffer.
	const maxToken = 1024 * 1024
	buf := make([]byte, 64*1024)
	scanner.Buffer(buf, maxToken)

	cutoff := time.Now().UTC().AddDate(0, 0, -minAgeDays)

	for scanner.Scan() {
		line := scanner.Bytes()

		var entry map[string]any
		if err := json.Unmarshal(line, &entry); err != nil {
			log.Printf("unparseable index line: %v", err)
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
		} else if _, err = w.Write(out); err != nil {
			break
		}
		if _, err = w.Write([]byte("\n")); err != nil {
			break
		}
	}

	if err := scanner.Err(); err != nil {
		log.Printf("scanner error: %v", err)
	}
}
