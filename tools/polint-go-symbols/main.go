package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"strings"

	"github.com/emilwareus/polint/tools/polint-go-symbols/internal/symbols"
)

func main() {
	if len(os.Args) < 2 || os.Args[1] != "symbols" {
		fmt.Fprintln(os.Stderr, "usage: polint-go-symbols symbols --root <path> --patterns <comma-list> --tests <bool> --build-tags <comma-list> --json")
		os.Exit(2)
	}

	flags := flag.NewFlagSet("symbols", flag.ExitOnError)
	root := flags.String("root", ".", "repository root")
	patterns := flags.String("patterns", "./...", "comma-separated package patterns")
	tests := flags.Bool("tests", true, "include test package variants")
	buildTags := flags.String("build-tags", "", "comma-separated Go build tags")
	jsonOutput := flags.Bool("json", false, "emit JSON")
	if err := flags.Parse(os.Args[2:]); err != nil {
		fmt.Fprintf(os.Stderr, "parse flags: %v\n", err)
		os.Exit(2)
	}
	if !*jsonOutput {
		fmt.Fprintln(os.Stderr, "--json is required")
		os.Exit(2)
	}

	out, err := symbols.Emit(symbols.Config{
		Root:         *root,
		Patterns:     splitComma(*patterns),
		IncludeTests: *tests,
		BuildTags:    splitComma(*buildTags),
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "emit Go symbols: %v\n", err)
		os.Exit(1)
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(out); err != nil {
		fmt.Fprintf(os.Stderr, "encode JSON: %v\n", err)
		os.Exit(1)
	}
}

func splitComma(value string) []string {
	var values []string
	for _, part := range strings.Split(value, ",") {
		part = strings.TrimSpace(part)
		if part != "" {
			values = append(values, part)
		}
	}
	return values
}
