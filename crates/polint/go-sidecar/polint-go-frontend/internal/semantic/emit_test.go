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

func TestEmitHarvestsRTASignals(t *testing.T) {
	root := writeFixture(t, map[string]string{
		"go.mod": "module example.test/fixture\n\ngo 1.24\n",
		"main.go": `package main

type I interface { M() }
type T struct{}

func (T) M() {}

func apply(f func()) { f() }

func call(i I) { i.M() }

func main() {
	var t T
	call(t)
	apply(func() { t.M() })
	apply(t.M)
}
`,
	})
	rows, err := Emit(Config{Root: root, ModuleRoots: []string{"."}, Patterns: []string{"./..."}, IncludeTests: false})
	if err != nil {
		t.Fatalf("Emit failed: %v", err)
	}
	assertKind(t, rows, "instantiated_type")
	assertKind(t, rows, "address_taken")
	assertKind(t, rows, "dynamic_dispatch")
	assertSchemaVersion(t, rows, "polint-go-semantic-2")
	assertDynamicDispatchJoinsCallsite(t, rows)
}

func TestEmitDynamicDispatchCarriesInterfaceDiscriminant(t *testing.T) {
	root := writeFixture(t, map[string]string{
		"go.mod": "module example.test/fixture\n\ngo 1.24\n",
		"main.go": `package main

type I interface { M() }
type T struct{}

func (T) M() {}
func call(i I) { i.M() }

func main() {
	var t T
	call(t)
}
`,
	})
	rows, err := Emit(Config{Root: root, ModuleRoots: []string{"."}, Patterns: []string{"./..."}, IncludeTests: false})
	if err != nil {
		t.Fatalf("Emit failed: %v", err)
	}
	found := false
	for _, row := range rows {
		if row["kind"] != "dynamic_dispatch" {
			continue
		}
		if row["method"] == "M" && row["interface_type"] != "" && row["interface_type"] != nil {
			found = true
		}
	}
	if !found {
		t.Fatalf("expected an interface-invoke dynamic_dispatch row with interface_type+method in %#v", rows)
	}
}

func TestEmitHonorsCheckedInGoWorkForMultipleModuleRoots(t *testing.T) {
	root := writeGoWorkspaceFixture(t, true)
	rows, err := Emit(Config{
		Root:         root,
		ModuleRoots:  []string{"services/app", "libs/money"},
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit failed: %v", err)
	}

	assertNoKind(t, rows, "package_error")
	assertPackagePath(t, rows, "example.test/app")
	assertPackagePath(t, rows, "example.test/money")
}

func TestEmitCreatesSyntheticGoWorkWhenRootGoWorkIsMissing(t *testing.T) {
	root := writeGoWorkspaceFixture(t, false)
	rows, err := Emit(Config{
		Root:         root,
		ModuleRoots:  []string{"services/app", "libs/money"},
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit failed: %v", err)
	}

	assertNoKind(t, rows, "package_error")
	assertPackagePath(t, rows, "example.test/app")
	assertPackagePath(t, rows, "example.test/money")
}

func TestEmitSpansUseAstRangesForFunctionsAndCallsites(t *testing.T) {
	root := writeFixture(t, map[string]string{
		"go.mod": "module example.test/fixture\n\ngo 1.24\n",
		"main.go": `package main

func helper() {}

func main() {
	helper()
}
`,
	})
	rows, err := Emit(Config{Root: root, ModuleRoots: []string{"."}, Patterns: []string{"./..."}, IncludeTests: false})
	if err != nil {
		t.Fatalf("Emit failed: %v", err)
	}

	assertRowSpanIsRange(t, rows, "function")
	assertRowSpanIsRange(t, rows, "callsite")
}

func TestInitFunctionNameDoesNotMatchInitialize(t *testing.T) {
	if isInitFunctionName("initialize") {
		t.Fatalf("initialize should stay a normal function")
	}
	if !isInitFunctionName("init") || !isInitFunctionName("init#1") || !isInitFunctionName("init$1") {
		t.Fatalf("expected SSA init names to be recognized")
	}
}

func TestSetEnvReplacesExistingValue(t *testing.T) {
	env := setEnv([]string{"PATH=/bin", "GOWORK=off", "HOME=/tmp"}, "GOWORK", "/tmp/go.work")

	var values []string
	for _, entry := range env {
		if len(entry) >= len("GOWORK=") && entry[:len("GOWORK=")] == "GOWORK=" {
			values = append(values, entry)
		}
	}
	if len(values) != 1 || values[0] != "GOWORK=/tmp/go.work" {
		t.Fatalf("expected replacement GOWORK, got %#v from env %#v", values, env)
	}
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

func assertNoKind(t *testing.T, rows []Row, kind string) {
	t.Helper()
	for _, row := range rows {
		if row["kind"] == kind {
			t.Fatalf("unexpected row kind %q in %#v", kind, rows)
		}
	}
}

func assertPackagePath(t *testing.T, rows []Row, packagePath string) {
	t.Helper()
	for _, row := range rows {
		if row["kind"] == "package" && row["package_path"] == packagePath {
			return
		}
	}
	t.Fatalf("missing package path %q in %#v", packagePath, rows)
}

func assertRowSpanIsRange(t *testing.T, rows []Row, kind string) {
	t.Helper()
	for _, row := range rows {
		if row["kind"] != kind {
			continue
		}
		span, ok := row["span"].(*Span)
		if !ok {
			continue
		}
		if span.EndByte <= span.StartByte {
			t.Fatalf("%s span should be a range, got %#v", kind, span)
		}
		return
	}
	t.Fatalf("missing %s row with span in %#v", kind, rows)
}

func assertSchemaVersion(t *testing.T, rows []Row, schema string) {
	t.Helper()
	for _, row := range rows {
		if row["schema"] != schema {
			t.Fatalf("row %#v has schema %v, expected %q", row, row["schema"], schema)
		}
	}
}

func assertDynamicDispatchJoinsCallsite(t *testing.T, rows []Row) {
	t.Helper()
	callsiteKeys := make(map[string]bool)
	for _, row := range rows {
		if row["kind"] == "callsite" {
			if key, ok := row["stable_key"].(string); ok {
				callsiteKeys[key] = true
			}
		}
	}
	for _, row := range rows {
		if row["kind"] != "dynamic_dispatch" {
			continue
		}
		key, ok := row["callsite_stable_key"].(string)
		if !ok || key == "" {
			t.Fatalf("dynamic_dispatch row missing callsite_stable_key: %#v", row)
		}
		if !callsiteKeys[key] {
			t.Fatalf("dynamic_dispatch callsite_stable_key %q has no matching callsite row", key)
		}
	}
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

func writeGoWorkspaceFixture(t *testing.T, checkedInWork bool) string {
	t.Helper()
	files := map[string]string{
		"services/app/go.mod": "module example.test/app\n\ngo 1.24\n\nrequire example.test/money v0.0.0\n",
		"services/app/main.go": `package main

import "example.test/money"

func main() {
	money.Pay()
}
`,
		"libs/money/go.mod": "module example.test/money\n\ngo 1.24\n",
		"libs/money/money.go": `package money

func Pay() {}
`,
	}
	if checkedInWork {
		files["go.work"] = "go 1.24\n\nuse (\n\t./services/app\n\t./libs/money\n)\n"
	}
	return writeFixture(t, files)
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
