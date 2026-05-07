package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"log"
	"net"
	"net/http"
	"time"
)

var (
	proxyAddr  = net.JoinHostPort("localhost", "2015")
	proxyBase  = fmt.Sprintf("http://%s", proxyAddr)
	minAgeDays = 14
)

func main() {
	mux := http.NewServeMux()
	mux.Handle("GET /index/config.json", http.HandlerFunc(configHandler))
	mux.Handle("GET /index/{path...}", http.HandlerFunc(indexHandler))

	log.Printf("cargo-age-proxy listening on %s", proxyAddr)
	log.Fatal(http.ListenAndServe(proxyAddr, logRequest(mux)))
}

func configHandler(w http.ResponseWriter, r *http.Request) {
	resp, err := http.Get("https://index.crates.io/config.json")
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	defer func() {
		_ = resp.Body.Close()
	}()

	cfg := struct {
		API string `json:"api"`
		DL  string `json:"dl"`
	}{}
	if err := json.NewDecoder(resp.Body).Decode(&cfg); err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	cfg.DL = proxyBase + "/crates/{crate}/{crate}-{version}.crate"

	w.Header().Set("Content-Type", "application/json")
	if err := json.NewEncoder(w).Encode(cfg); err != nil {
		log.Printf("json encode error: %v", err)
	}
}

func indexHandler(w http.ResponseWriter, r *http.Request) {
	path := r.PathValue("path")
	resp, err := http.Get("https://index.crates.io/" + path)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	defer func() {
		_ = resp.Body.Close()
	}()

	w.Header().Set("Content-Type", resp.Header.Get("Content-Type"))
	w.WriteHeader(resp.StatusCode)

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
			http.Error(w, "Error marshaling upstream response", http.StatusInternalServerError)
			return
		} else {
			if _, err = w.Write(out); err != nil {
				return
			}
		}
		if _, err = w.Write([]byte("\n")); err != nil {
			return
		}
	}

	if err := scanner.Err(); err != nil {
		log.Printf("scanner error: %v", err)
	}
}

// Source: https://stackoverflow.com/a/53272925 (huangapple, CC BY-SA 4.0)
type logResponseWriter struct {
	http.ResponseWriter
	statusCode int
}

func newLogResponseWriter(w http.ResponseWriter) *logResponseWriter {
	return &logResponseWriter{w, http.StatusOK}
}

func (w *logResponseWriter) WriteHeader(code int) {
	w.statusCode = code
	w.ResponseWriter.WriteHeader(code)
}

func logRequest(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		lw := newLogResponseWriter(w)
		next.ServeHTTP(lw, r)
		log.Printf("%s %s %d %s", r.Method, r.URL.Path, lw.statusCode, http.StatusText(lw.statusCode))
	})
}
