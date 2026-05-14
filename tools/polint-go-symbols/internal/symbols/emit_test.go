package symbols

import (
	"encoding/json"
	"os"
	"path/filepath"
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
