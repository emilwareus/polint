package symbols

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestEmitLoadsTypedSymbolsDefinitionsAndReferences(t *testing.T) {
	root := writeModule(t, map[string]string{
		"go.mod": "module example.com/app\n\ngo 1.24.0\n",
		"widget.go": `package app

type Widget struct {
	Name string
}

func Build() *Widget {
	return &Widget{Name: "ok"}
}

func Use() string {
	w := Build()
	return w.Name
}
`,
	})

	out, err := Emit(Config{
		Root:         root,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit returned error: %v", err)
	}

	if out.Schema != SchemaVersion {
		t.Fatalf("schema = %q, want %q", out.Schema, SchemaVersion)
	}

	build := symbolByName(out.Symbols, "Build")
	if build == nil {
		t.Fatalf("Build symbol missing from %#v", out.Symbols)
	}
	if build.ObjectPath == "" {
		t.Fatalf("Build objectpath is empty: %#v", build)
	}
	if build.Kind != "function" || build.Namespace != "value" {
		t.Fatalf("Build kind/namespace = %s/%s", build.Kind, build.Namespace)
	}
	if !hasDefinition(out.Definitions, build.Key) {
		t.Fatalf("Build definition missing from %#v", out.Definitions)
	}
	if !hasReference(out.References, build.Key, "call") {
		t.Fatalf("Build call reference missing from %#v", out.References)
	}

	field := symbolByName(out.Symbols, "Name")
	if field == nil || field.Kind != "field" {
		t.Fatalf("Name field symbol missing from %#v", out.Symbols)
	}
	if !hasReference(out.References, field.Key, "field") {
		t.Fatalf("Name field selector reference missing from %#v", out.References)
	}
}

func TestEmitSemanticRowsForScopesImportsAndExports(t *testing.T) {
	root := writeModule(t, map[string]string{
		"go.mod": "module example.com/app\n\ngo 1.24.0\n",
		"widget.go": `package app

import (
	named "fmt"
	. "strings"
	_ "net/http/pprof"
)

type Widget struct {
	Name string
}

func (w Widget) Label() string {
	if w.Name == "" {
		return named.Sprint(TrimSpace("fallback"))
	}
	return w.Name
}
`,
	})

	out, err := Emit(Config{
		Root:         root,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit returned error: %v", err)
	}

	if out.Schema != "polint-go-symbols-semantic-1" {
		t.Fatalf("schema = %q, want semantic schema", out.Schema)
	}
	for _, kind := range []string{"package", "file", "type", "method", "block"} {
		if !hasScopeKind(out.Scopes, kind) {
			t.Fatalf("scope kind %q missing from %#v", kind, out.Scopes)
		}
	}
	for _, alias := range []string{"named", "dot", "blank"} {
		if !hasImportAliasKind(out.Imports, alias) {
			t.Fatalf("import alias kind %q missing from %#v", alias, out.Imports)
		}
	}
	if !hasExportObjectPath(out.Exports, "Widget") {
		t.Fatalf("Widget export with object path missing from %#v", out.Exports)
	}
	if !hasExportObjectPath(out.Exports, "Label") {
		t.Fatalf("Label export with object path missing from %#v", out.Exports)
	}
}

func TestEmitScopeKeysUseFileRelativeOffsets(t *testing.T) {
	widget := `package app

type Widget struct {
	Name string
}

func Use() string {
	w := Widget{Name: "ok"}
	return w.Name
}
`
	baseRoot := writeModule(t, map[string]string{
		"go.mod":    "module example.com/app\n\ngo 1.24.0\n",
		"widget.go": widget,
	})
	expandedRoot := writeModule(t, map[string]string{
		"go.mod": "module example.com/app\n\ngo 1.24.0\n",
		"aaa.go": `package app

func Earlier() int {
	return 1
}
`,
		"widget.go": widget,
	})

	base, err := Emit(Config{
		Root:         baseRoot,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("base Emit returned error: %v", err)
	}
	expanded, err := Emit(Config{
		Root:         expandedRoot,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("expanded Emit returned error: %v", err)
	}

	baseKeys := scopeKeysForFile(base.Scopes, "widget.go")
	expandedKeys := scopeKeysForFile(expanded.Scopes, "widget.go")
	if !reflect.DeepEqual(baseKeys, expandedKeys) {
		t.Fatalf("widget.go scope keys changed after adding unrelated file:\nbase: %#v\nexpanded: %#v", baseKeys, expandedKeys)
	}
	for _, key := range expandedKeys {
		if strings.Contains(key, "pos:") || !strings.Contains(key, "offset:") {
			t.Fatalf("scope key %q should use file-relative offset labels", key)
		}
	}
}

func TestEmitClassifiesAssignmentReferences(t *testing.T) {
	root := writeModule(t, map[string]string{
		"go.mod": "module example.com/app\n\ngo 1.24.0\n",
		"widget.go": `package app

type Widget struct {
	Count int
}

func Use(w Widget) int {
	value := 1
	value = 2
	value += 3
	value++
	w.Count = value
	w.Count++
	return value + w.Count
}
`,
	})

	out, err := Emit(Config{
		Root:         root,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit returned error: %v", err)
	}

	value := symbolByName(out.Symbols, "value")
	if value == nil {
		t.Fatalf("value symbol missing from %#v", out.Symbols)
	}
	if !hasReference(out.References, value.Key, "write") {
		t.Fatalf("value write reference missing from %#v", out.References)
	}
	if !hasReference(out.References, value.Key, "read_write") {
		t.Fatalf("value read_write reference missing from %#v", out.References)
	}

	count := symbolByName(out.Symbols, "Count")
	if count == nil {
		t.Fatalf("Count symbol missing from %#v", out.Symbols)
	}
	if !hasReference(out.References, count.Key, "write") {
		t.Fatalf("Count write reference missing from %#v", out.References)
	}
	if !hasReference(out.References, count.Key, "read_write") {
		t.Fatalf("Count read_write reference missing from %#v", out.References)
	}
}

func TestEmitClassifiesPackageQualifiedCallsAsExternalCalls(t *testing.T) {
	root := writeModule(t, map[string]string{
		"go.mod": "module example.com/app\n\ngo 1.24.0\n",
		"main.go": `package app

import "fmt"

func Use() {
	fmt.Println("ok")
}
`,
	})

	out, err := Emit(Config{
		Root:         root,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit returned error: %v", err)
	}

	println := symbolByName(out.Symbols, "Println")
	if println == nil {
		t.Fatalf("Println symbol missing from %#v", out.Symbols)
	}
	if println.File != "" {
		t.Fatalf("Println file = %q, want external symbol without a local file", println.File)
	}
	if println.PackagePath != "fmt" {
		t.Fatalf("Println package path = %q, want fmt", println.PackagePath)
	}
	if println.QualifiedName != "fmt.Println" {
		t.Fatalf("Println qualified name = %q, want fmt.Println", println.QualifiedName)
	}
	if !hasReference(out.References, println.Key, "call") {
		t.Fatalf("Println call reference missing from %#v", out.References)
	}
	if hasReference(out.References, println.Key, "read") {
		t.Fatalf("Println was also emitted as a read reference: %#v", out.References)
	}
}

func TestEmitLocalSymbolKeysIncludePackageFileOwnerNameAndPosition(t *testing.T) {
	root := writeModule(t, map[string]string{
		"go.mod": "module example.com/app\n\ngo 1.24.0\n",
		"widget.go": `package app

func Use() int {
	local := 41
	return local + 1
}
`,
	})

	out, err := Emit(Config{
		Root:         root,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit returned error: %v", err)
	}

	local := symbolByName(out.Symbols, "local")
	if local == nil {
		t.Fatalf("local symbol missing from %#v", out.Symbols)
	}
	keyParts := []string{
		"example.com/app",
		"widget.go",
		"Use",
		"local",
		"line:",
		"column:",
	}
	for _, part := range keyParts {
		if !strings.Contains(local.Key, part) {
			t.Fatalf("local key %q missing %q", local.Key, part)
		}
	}
	if local.ObjectPath != "" {
		t.Fatalf("local objectpath = %q, want empty", local.ObjectPath)
	}
}

func TestEmitJSONDoesNotContainRawSourceText(t *testing.T) {
	root := writeModule(t, map[string]string{
		"go.mod": "module example.com/app\n\ngo 1.24.0\n",
		"widget.go": `package app

func Build() string {
	return "raw-source-sentinel"
}
`,
	})

	out, err := Emit(Config{
		Root:         root,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit returned error: %v", err)
	}

	encoded, err := json.Marshal(out)
	if err != nil {
		t.Fatalf("marshal output: %v", err)
	}
	text := string(encoded)
	for _, forbidden := range []string{"raw-source-sentinel", "return \"raw-source-sentinel\"", "func Build()", "package app", `"Source"`, `"Content"`} {
		if strings.Contains(text, forbidden) {
			t.Fatalf("sidecar JSON contains raw source marker %q in %s", forbidden, text)
		}
	}
}

func TestEmitJSONUsesArraysForEmptyCollections(t *testing.T) {
	root := writeModule(t, map[string]string{
		"go.mod": "module example.com/app\n\ngo 1.24.0\n",
	})

	out, err := Emit(Config{
		Root:         root,
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit returned error: %v", err)
	}

	encoded, err := json.Marshal(out)
	if err != nil {
		t.Fatalf("marshal output: %v", err)
	}
	text := string(encoded)
	for _, expected := range []string{`"packages":[]`, `"symbols":[]`, `"definitions":[]`, `"references":[]`} {
		if !strings.Contains(text, expected) {
			t.Fatalf("sidecar JSON missing empty array %s in %s", expected, text)
		}
	}
	if strings.Contains(text, ":null") {
		t.Fatalf("sidecar JSON should not contain null sequence fields: %s", text)
	}
}

func TestGoWorkUsePathPrefersPathRelativeToWorkspaceFile(t *testing.T) {
	parent := t.TempDir()
	root := filepath.Join(parent, "repo")
	moduleRoot := filepath.Join(root, "services", "app")

	got := goWorkUsePath(parent, moduleRoot)
	want := filepath.ToSlash(filepath.Join("repo", "services", "app"))
	if got != want {
		t.Fatalf("goWorkUsePath() = %q, want %q", got, want)
	}
}

func TestEmitUsesSyntheticWorkspaceWhenRootGoWorkMissesConfiguredRoots(t *testing.T) {
	root := writeModule(t, map[string]string{
		"go.work":               "go 1.24.0\n\nuse ./tools/only\n",
		"tools/only/go.mod":     "module example.com/tools\n\ngo 1.24.0\n",
		"tools/only/tool.go":    "package tool\n",
		"libs/shared/go.mod":    "module example.com/shared\n\ngo 1.24.0\n",
		"libs/shared/shared.go": "package shared\n\nfunc Build() string { return \"ok\" }\n",
		"services/app/go.mod": `module example.com/app

go 1.24.0

require example.com/shared v0.0.0
`,
		"services/app/main.go": `package app

import "example.com/shared"

func Use() string {
	return shared.Build()
}
`,
	})

	out, err := Emit(Config{
		Root:         root,
		ModuleRoots:  []string{"services/app", "libs/shared"},
		Patterns:     []string{"./..."},
		IncludeTests: false,
	})
	if err != nil {
		t.Fatalf("Emit returned error: %v", err)
	}

	build := symbolByName(out.Symbols, "Build")
	if build == nil {
		t.Fatalf("Build symbol missing from %#v", out.Symbols)
	}
	if !hasReference(out.References, build.Key, "call") {
		t.Fatalf("Build call reference missing from %#v", out.References)
	}
}

func writeModule(t *testing.T, files map[string]string) string {
	t.Helper()

	root := t.TempDir()
	for name, content := range files {
		path := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatalf("mkdir %s: %v", filepath.Dir(path), err)
		}
		if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
			t.Fatalf("write %s: %v", path, err)
		}
	}
	return root
}

func symbolByName(symbols []SymbolRow, name string) *SymbolRow {
	for i := range symbols {
		if symbols[i].Name == name {
			return &symbols[i]
		}
	}
	return nil
}

func hasDefinition(definitions []DefinitionRow, symbolKey string) bool {
	for _, definition := range definitions {
		if definition.SymbolKey == symbolKey {
			return true
		}
	}
	return false
}

func hasReference(references []ReferenceRow, targetKey string, kind string) bool {
	for _, reference := range references {
		if reference.TargetKey == targetKey && reference.Kind == kind {
			return true
		}
	}
	return false
}

func hasScopeKind(scopes []ScopeRow, kind string) bool {
	for _, scope := range scopes {
		if scope.Kind == kind {
			return true
		}
	}
	return false
}

func scopeKeysForFile(scopes []ScopeRow, file string) []string {
	keys := make([]string, 0)
	for _, scope := range scopes {
		if scope.File == file {
			keys = append(keys, scope.Key)
		}
	}
	return keys
}

func hasImportAliasKind(imports []ImportRow, alias string) bool {
	for _, imp := range imports {
		if imp.AliasKind == alias {
			return true
		}
	}
	return false
}

func hasExportObjectPath(exports []ExportRow, name string) bool {
	for _, export := range exports {
		if export.ExportName == name && export.ObjectPath != "" && export.PackagePath == "example.com/app" {
			return true
		}
	}
	return false
}
