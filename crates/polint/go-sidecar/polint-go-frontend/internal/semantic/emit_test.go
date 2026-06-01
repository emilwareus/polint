package semantic

import (
	"os"
	"path/filepath"
	"testing"
)

func TestValidatePackagePatternsRejectsFlags(t *testing.T) {
	_, err := validatePackagePatterns([]string{"-json"})
	if err == nil || err.Error() != "package pattern \"-json\" must not start with -" {
		t.Fatalf("expected flag-like pattern rejection, got %v", err)
	}
}

func TestEmitLoadsPackagesAndBuildsSSA(t *testing.T) {
	root := writeFixture(t, map[string]string{
		"go.mod": "module example.test/fixture\n\ngo 1.24\n",
		"main.go": `package main

func helper() {}

func main() {
	helper()
}
`,
	})
	rows, err := Emit(Config{Root: root, ModuleRoots: []string{"."}, Patterns: []string{"./..."}, IncludeTests: true})
	if err != nil {
		t.Fatalf("Emit failed: %v", err)
	}
	assertKind(t, rows, "package")
	assertKind(t, rows, "function")
	assertKind(t, rows, "callsite")
}

func TestEmitNDJSONRows(t *testing.T) {
	root := writeFixture(t, map[string]string{
		"go.mod": "module example.test/fixture\n\ngo 1.24\n",
		"main.go": `package main

type T struct{}

func (T) M() {}
func main() {
	var t T
	t.M()
}
`,
	})
	rows, err := Emit(Config{Root: root, ModuleRoots: []string{"."}, Patterns: []string{"./..."}, IncludeTests: true})
	if err != nil {
		t.Fatalf("Emit failed: %v", err)
	}
	if rows[0]["kind"] != "session_begin" {
		t.Fatalf("first row kind = %v", rows[0]["kind"])
	}
	if rows[len(rows)-1]["kind"] != "session_end" {
		t.Fatalf("last row kind = %v", rows[len(rows)-1]["kind"])
	}
	assertKind(t, rows, "package")
	assertKind(t, rows, "function")
	assertKind(t, rows, "callsite")
	assertKind(t, rows, "method_set")
}

func TestEmitCoversMethodsReceiversInitAndUnsupported(t *testing.T) {
	root := writeFixture(t, map[string]string{
		"go.mod": "module example.test/fixture\n\ngo 1.24\n",
		"main.go": `package main

type I interface { M() }
type T struct{}

func init() {}
func (T) M() {}
func call(i I) { i.M() }
func main() {
	var t T
	call(t)
}
`,
	})
	rows, err := Emit(Config{Root: root, ModuleRoots: []string{"."}, Patterns: []string{"./..."}, IncludeTests: true})
	if err != nil {
		t.Fatalf("Emit failed: %v", err)
	}
	assertKind(t, rows, "method")
	assertKind(t, rows, "receiver_type")
	assertKind(t, rows, "init_function")
	assertKind(t, rows, "method_set")
	assertKind(t, rows, "callsite")
	assertCallsiteStatus(t, rows, "unresolved_dynamic")
}

func assertKind(t *testing.T, rows []Row, kind string) {
	t.Helper()
	for _, row := range rows {
		if row["kind"] == kind {
			return
		}
	}
	t.Fatalf("missing row kind %q in %#v", kind, rows)
}

func assertCallsiteStatus(t *testing.T, rows []Row, status string) {
	t.Helper()
	for _, row := range rows {
		if row["kind"] == "callsite" && row["status"] == status {
			return
		}
	}
	t.Fatalf("missing callsite status %q in %#v", status, rows)
}

func writeFixture(t *testing.T, files map[string]string) string {
	t.Helper()
	root := t.TempDir()
	for relative, contents := range files {
		path := filepath.Join(root, filepath.FromSlash(relative))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatalf("mkdir fixture: %v", err)
		}
		if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
			t.Fatalf("write fixture: %v", err)
		}
	}
	return root
}
