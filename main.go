package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net"
	"net/http"
)

var (
	proxyAddr = net.JoinHostPort("localhost", "2015")
	proxyBase = fmt.Sprintf("http://%s", proxyAddr)
)

func main() {
	mux := http.NewServeMux()
	mux.Handle("GET /index/config.json", http.HandlerFunc(configHandler))

	log.Printf("cargo-age-proxy listening on %s", proxyAddr)
	log.Fatal(http.ListenAndServe(proxyAddr, logRequest(mux)))
}

func configHandler(w http.ResponseWriter, r *http.Request) {
	resp, err := http.Get("https://index.crates.io/config.json")
	if err != nil {
		log.Printf("upstream error: %v", err)
		http.Error(w, "upstream error", http.StatusBadGateway)
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
		log.Printf("json decode error: %v", err)
		http.Error(w, "invalid upstream response", http.StatusBadGateway)
		return
	}
	cfg.DL = proxyBase + "/crates/{crate}/{crate}-{version}.crate"

	w.Header().Set("Content-Type", "application/json")
	if err := json.NewEncoder(w).Encode(cfg); err != nil {
		log.Printf("json encode error: %v", err)
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
