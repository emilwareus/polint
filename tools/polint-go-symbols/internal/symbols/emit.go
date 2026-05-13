package symbols

import (
	"fmt"
	"go/ast"
	"go/token"
	"go/types"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"

	"golang.org/x/tools/go/packages"
	"golang.org/x/tools/go/types/objectpath"
)

const SchemaVersion = "polint-go-symbols-v1"

type Config struct {
	Root         string
	Patterns     []string
	IncludeTests bool
	BuildTags    []string
}

type Output struct {
	Schema      string          `json:"schema"`
	GoVersion   string          `json:"go_version"`
	ModulePath  string          `json:"module_path,omitempty"`
	Packages    []PackageRow    `json:"packages"`
	Symbols     []SymbolRow     `json:"symbols"`
	Definitions []DefinitionRow `json:"definitions"`
	References  []ReferenceRow  `json:"references"`
	Errors      []PackageError  `json:"errors,omitempty"`
}

type PackageRow struct {
	ID          string   `json:"id"`
	Path        string   `json:"path"`
	Name        string   `json:"name"`
	ModulePath  string   `json:"module_path,omitempty"`
	TestVariant string   `json:"test_variant"`
	Files       []string `json:"files"`
}

type SymbolRow struct {
	Key           string   `json:"key"`
	PackageID     string   `json:"package_id"`
	PackagePath   string   `json:"package_path"`
	TestVariant   string   `json:"test_variant"`
	File          string   `json:"file,omitempty"`
	OwnerKey      string   `json:"owner_key,omitempty"`
	OwnerChain    []string `json:"owner_chain,omitempty"`
	Name          string   `json:"name"`
	QualifiedName string   `json:"qualified_name"`
	Namespace     string   `json:"namespace"`
	Kind          string   `json:"kind"`
	ObjectPath    string   `json:"objectpath,omitempty"`
	Span          Span     `json:"span"`
	Exported      bool     `json:"exported"`
}

type DefinitionRow struct {
	SymbolKey string `json:"symbol_key"`
	PackageID string `json:"package_id"`
	File      string `json:"file,omitempty"`
	Name      string `json:"name"`
	Kind      string `json:"kind"`
	Span      Span   `json:"span"`
	Implicit  bool   `json:"implicit"`
	Primary   bool   `json:"primary"`
}

type ReferenceRow struct {
	PackageID string `json:"package_id"`
	File      string `json:"file,omitempty"`
	Name      string `json:"name"`
	TargetKey string `json:"target_key,omitempty"`
	Kind      string `json:"kind"`
	Span      Span   `json:"span"`
	Precision string `json:"precision"`
}

type PackageError struct {
	PackageID   string `json:"package_id"`
	PackagePath string `json:"package_path"`
	Message     string `json:"message"`
}

type Span struct {
	StartByte   int `json:"start_byte"`
	EndByte     int `json:"end_byte"`
	StartLine   int `json:"start_line"`
	StartColumn int `json:"start_column"`
	EndLine     int `json:"end_line"`
	EndColumn   int `json:"end_column"`
}

type emitter struct {
	root        string
	fset        *token.FileSet
	out         Output
	symbols     map[types.Object]string
	symbolRows  map[string]SymbolRow
	defRows     map[string]DefinitionRow
	refRows     map[string]ReferenceRow
	kindByPos   map[token.Pos]string
	parents     map[ast.Node]ast.Node
	ownerRanges []ownerRange
}

type ownerRange struct {
	start token.Pos
	end   token.Pos
	name  string
}

func Emit(config Config) (Output, error) {
	root, err := filepath.Abs(config.Root)
	if err != nil {
		return Output{}, fmt.Errorf("resolve root: %w", err)
	}
	patterns := config.Patterns
	if len(patterns) == 0 {
		patterns = []string{"./..."}
	}

	fset := token.NewFileSet()
	loadConfig := &packages.Config{
		Mode: packages.NeedName |
			packages.NeedFiles |
			packages.NeedCompiledGoFiles |
			packages.NeedImports |
			packages.NeedSyntax |
			packages.NeedTypes |
			packages.NeedTypesInfo |
			packages.NeedTypesSizes |
			packages.NeedModule,
		Dir:   root,
		Fset:  fset,
		Tests: config.IncludeTests,
		Env:   goPackageEnv(root),
	}
	if len(config.BuildTags) > 0 {
		loadConfig.BuildFlags = []string{"-tags=" + strings.Join(config.BuildTags, ",")}
	}

	pkgs, err := packages.Load(loadConfig, patterns...)
	if err != nil {
		return Output{}, err
	}
	sort.Slice(pkgs, func(i, j int) bool {
		return pkgs[i].ID < pkgs[j].ID
	})

	e := &emitter{
		root: root,
		fset: fset,
		out: Output{
			Schema:    SchemaVersion,
			GoVersion: runtime.Version(),
		},
		symbols:    make(map[types.Object]string),
		symbolRows: make(map[string]SymbolRow),
		defRows:    make(map[string]DefinitionRow),
		refRows:    make(map[string]ReferenceRow),
		kindByPos:  make(map[token.Pos]string),
		parents:    make(map[ast.Node]ast.Node),
	}

	for _, pkg := range pkgs {
		if e.out.ModulePath == "" && pkg.Module != nil {
			e.out.ModulePath = pkg.Module.Path
		}
		e.indexSyntax(pkg)
		e.emitPackage(pkg)
		e.emitPackageErrors(pkg)
		e.emitDefinitions(pkg)
		e.emitImplicits(pkg)
		e.emitUses(pkg)
		e.emitSelections(pkg)
	}
	e.finish()
	return e.out, nil
}

func goPackageEnv(root string) []string {
	env := os.Environ()
	gowork := "off"
	if path := filepath.Join(root, "go.work"); fileExists(path) {
		gowork = path
	}
	env = append(env, "GOWORK="+gowork)
	return env
}

func fileExists(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir()
}

func (e *emitter) indexSyntax(pkg *packages.Package) {
	for _, file := range pkg.Syntax {
		e.indexParents(file)
		e.indexKinds(file)
		e.indexOwners(file)
	}
}

func (e *emitter) indexParents(file *ast.File) {
	var stack []ast.Node
	ast.Inspect(file, func(node ast.Node) bool {
		if node == nil {
			if len(stack) > 0 {
				stack = stack[:len(stack)-1]
			}
			return false
		}
		if len(stack) > 0 {
			e.parents[node] = stack[len(stack)-1]
		}
		stack = append(stack, node)
		return true
	})
}

func (e *emitter) indexKinds(file *ast.File) {
	ast.Inspect(file, func(node ast.Node) bool {
		switch n := node.(type) {
		case *ast.FuncType:
			markFieldListKinds(e.kindByPos, n.Params, "parameter")
			markFieldListKinds(e.kindByPos, n.Results, "parameter")
		case *ast.StructType:
			markFieldListKinds(e.kindByPos, n.Fields, "field")
		}
		return true
	})
}

func (e *emitter) indexOwners(file *ast.File) {
	ast.Inspect(file, func(node ast.Node) bool {
		switch n := node.(type) {
		case *ast.FuncDecl:
			name := "func " + n.Name.Name
			if recv := receiverName(n.Recv); recv != "" {
				name = "method " + recv + "." + n.Name.Name
			}
			e.ownerRanges = append(e.ownerRanges, ownerRange{start: n.Pos(), end: n.End(), name: name})
		case *ast.FuncLit:
			span := e.span(n.Type.Pos(), n.Type.Pos()+token.Pos(len("func")))
			e.ownerRanges = append(e.ownerRanges, ownerRange{
				start: n.Pos(),
				end:   n.End(),
				name:  fmt.Sprintf("func@line:%d:column:%d", span.StartLine, span.StartColumn),
			})
		case *ast.TypeSpec:
			e.ownerRanges = append(e.ownerRanges, ownerRange{start: n.Pos(), end: n.End(), name: "type " + n.Name.Name})
		}
		return true
	})
}

func markFieldListKinds(kinds map[token.Pos]string, fields *ast.FieldList, kind string) {
	if fields == nil {
		return
	}
	for _, field := range fields.List {
		for _, name := range field.Names {
			kinds[name.Pos()] = kind
		}
	}
}

func (e *emitter) emitPackage(pkg *packages.Package) {
	files := make([]string, 0, len(pkg.CompiledGoFiles))
	for _, file := range pkg.CompiledGoFiles {
		if rel := e.relativePath(file); rel != "" {
			files = append(files, rel)
		}
	}
	sort.Strings(files)
	modulePath := ""
	if pkg.Module != nil {
		modulePath = pkg.Module.Path
	}
	e.out.Packages = append(e.out.Packages, PackageRow{
		ID:          pkg.ID,
		Path:        pkg.PkgPath,
		Name:        pkg.Name,
		ModulePath:  modulePath,
		TestVariant: testVariant(pkg),
		Files:       files,
	})
}

func (e *emitter) emitPackageErrors(pkg *packages.Package) {
	for _, err := range pkg.Errors {
		e.out.Errors = append(e.out.Errors, PackageError{
			PackageID:   pkg.ID,
			PackagePath: pkg.PkgPath,
			Message:     err.Msg,
		})
	}
}

func (e *emitter) emitDefinitions(pkg *packages.Package) {
	type item struct {
		ident *ast.Ident
		obj   types.Object
	}
	items := make([]item, 0, len(pkg.TypesInfo.Defs))
	for ident, obj := range pkg.TypesInfo.Defs {
		if ident != nil && obj != nil {
			items = append(items, item{ident: ident, obj: obj})
		}
	}
	sort.Slice(items, func(i, j int) bool {
		return posLess(items[i].ident.Pos(), items[j].ident.Pos(), items[i].ident.Name, items[j].ident.Name)
	})
	for _, item := range items {
		key := e.ensureSymbol(pkg, item.ident, item.obj)
		if key == "" {
			continue
		}
		span := e.identSpan(item.ident)
		e.defRows[definitionKey(key, span, false)] = DefinitionRow{
			SymbolKey: key,
			PackageID: pkg.ID,
			File:      e.fileForPos(item.ident.Pos()),
			Name:      item.ident.Name,
			Kind:      definitionKind(item.obj),
			Span:      span,
			Implicit:  false,
			Primary:   true,
		}
	}
}

func (e *emitter) emitImplicits(pkg *packages.Package) {
	type item struct {
		node ast.Node
		obj  types.Object
	}
	items := make([]item, 0, len(pkg.TypesInfo.Implicits))
	for node, obj := range pkg.TypesInfo.Implicits {
		if node != nil && obj != nil {
			items = append(items, item{node: node, obj: obj})
		}
	}
	sort.Slice(items, func(i, j int) bool {
		return posLess(items[i].node.Pos(), items[j].node.Pos(), items[i].obj.Name(), items[j].obj.Name())
	})
	for _, item := range items {
		ident := ast.NewIdent(item.obj.Name())
		ident.NamePos = item.node.Pos()
		key := e.ensureSymbol(pkg, ident, item.obj)
		if key == "" {
			continue
		}
		span := e.span(item.node.Pos(), item.node.End())
		e.defRows[definitionKey(key, span, true)] = DefinitionRow{
			SymbolKey: key,
			PackageID: pkg.ID,
			File:      e.fileForPos(item.node.Pos()),
			Name:      item.obj.Name(),
			Kind:      "implicit",
			Span:      span,
			Implicit:  true,
			Primary:   false,
		}
	}
}

func (e *emitter) emitUses(pkg *packages.Package) {
	type item struct {
		ident *ast.Ident
		obj   types.Object
	}
	items := make([]item, 0, len(pkg.TypesInfo.Uses))
	for ident, obj := range pkg.TypesInfo.Uses {
		if ident != nil && obj != nil {
			items = append(items, item{ident: ident, obj: obj})
		}
	}
	sort.Slice(items, func(i, j int) bool {
		return posLess(items[i].ident.Pos(), items[j].ident.Pos(), items[i].ident.Name, items[j].ident.Name)
	})
	for _, item := range items {
		if selector := e.selectorParent(item.ident); selector != nil {
			if _, ok := pkg.TypesInfo.Selections[selector]; ok && selector.Sel == item.ident {
				continue
			}
		}
		targetKey := e.ensureSymbol(pkg, item.ident, item.obj)
		kind := e.referenceKind(item.ident, item.obj)
		e.addReference(pkg.ID, e.fileForPos(item.ident.Pos()), item.ident.Name, targetKey, kind, e.identSpan(item.ident))
	}
}

func (e *emitter) emitSelections(pkg *packages.Package) {
	type item struct {
		selector  *ast.SelectorExpr
		selection *types.Selection
	}
	items := make([]item, 0, len(pkg.TypesInfo.Selections))
	for selector, selection := range pkg.TypesInfo.Selections {
		if selector != nil && selection != nil && selection.Obj() != nil {
			items = append(items, item{selector: selector, selection: selection})
		}
	}
	sort.Slice(items, func(i, j int) bool {
		return posLess(items[i].selector.Sel.Pos(), items[j].selector.Sel.Pos(), items[i].selector.Sel.Name, items[j].selector.Sel.Name)
	})
	for _, item := range items {
		targetKey := e.ensureSymbol(pkg, item.selector.Sel, item.selection.Obj())
		kind := e.selectionReferenceKind(item.selector, item.selection)
		e.addReference(pkg.ID, e.fileForPos(item.selector.Sel.Pos()), item.selector.Sel.Name, targetKey, kind, e.identSpan(item.selector.Sel))
	}
}

func (e *emitter) ensureSymbol(pkg *packages.Package, ident *ast.Ident, obj types.Object) string {
	if obj == nil {
		return ""
	}
	if obj.Name() == "" {
		return ""
	}
	if key, ok := e.symbols[obj]; ok {
		return key
	}
	if ident == nil {
		ident = ast.NewIdent(obj.Name())
		ident.NamePos = obj.Pos()
	}
	span := e.identSpan(ident)
	file := e.fileForPos(ident.Pos())
	if obj.Pkg() == nil {
		file = ""
	}
	kind := e.symbolKind(obj, ident)
	namespace := symbolNamespace(obj)
	objectPath, hasObjectPath := safeObjectPath(obj)
	ownerChain := e.ownerChainAt(pkg, ident.Pos())
	if obj.Pkg() == nil {
		ownerChain = nil
	}
	ownerKey := ""
	key := e.symbolKey(pkg, obj, file, kind, namespace, objectPath, hasObjectPath, ownerChain, span)
	if key == "" {
		return ""
	}

	row := SymbolRow{
		Key:           key,
		PackageID:     pkg.ID,
		PackagePath:   pkg.PkgPath,
		TestVariant:   testVariant(pkg),
		File:          file,
		OwnerKey:      ownerKey,
		OwnerChain:    ownerChain,
		Name:          obj.Name(),
		QualifiedName: qualifiedName(obj),
		Namespace:     namespace,
		Kind:          kind,
		ObjectPath:    objectPath,
		Span:          span,
		Exported:      obj.Exported(),
	}
	e.symbols[obj] = key
	e.symbolRows[key] = row
	return key
}

func (e *emitter) symbolKind(obj types.Object, ident *ast.Ident) string {
	switch typed := obj.(type) {
	case *types.Func:
		if signature, ok := typed.Type().(*types.Signature); ok && signature.Recv() != nil {
			return "method"
		}
		return "function"
	case *types.Var:
		if typed.IsField() {
			return "field"
		}
		if e.kindByPos[ident.Pos()] == "parameter" {
			return "parameter"
		}
		return "variable"
	case *types.Const:
		return "constant"
	case *types.TypeName:
		return "type"
	case *types.PkgName:
		return "package"
	case *types.Label:
		return "label"
	default:
		return "unknown"
	}
}

func symbolNamespace(obj types.Object) string {
	switch obj.(type) {
	case *types.TypeName:
		return "type"
	case *types.PkgName:
		return "package"
	case *types.Label:
		return "label"
	default:
		return "value"
	}
}

func (e *emitter) symbolKey(pkg *packages.Package, obj types.Object, file string, kind string, namespace string, objectPath string, hasObjectPath bool, ownerChain []string, span Span) string {
	var pkgPath string
	if obj.Pkg() != nil {
		pkgPath = obj.Pkg().Path()
	}
	if pkgPath == "" {
		pkgPath = pkg.PkgPath
	}
	if hasObjectPath {
		return strings.Join([]string{
			"go:package",
			"module:" + modulePath(pkg),
			"package:" + pkgPath,
			"variant:" + testVariant(pkg),
			"namespace:" + namespace,
			"kind:" + kind,
			"name:" + obj.Name(),
			"objectpath:" + objectPath,
		}, "|")
	}
	if obj.Pkg() == nil && file == "" {
		return strings.Join([]string{
			"go:builtin",
			"namespace:" + namespace,
			"kind:" + kind,
			"name:" + obj.Name(),
		}, "|")
	}
	if isPackageLevel(obj, pkg) {
		return strings.Join([]string{
			"go:package",
			"module:" + modulePath(pkg),
			"package:" + pkgPath,
			"variant:" + testVariant(pkg),
			"namespace:" + namespace,
			"kind:" + kind,
			"name:" + obj.Name(),
			fmt.Sprintf("line:%d", span.StartLine),
			fmt.Sprintf("column:%d", span.StartColumn),
		}, "|")
	}
	parts := []string{
		"go:local",
		"package_id:" + pkg.ID,
		"package:" + pkgPath,
		"variant:" + testVariant(pkg),
		"file:" + file,
		"owner:" + strings.Join(ownerChain, "/"),
		"namespace:" + namespace,
		"kind:" + kind,
		"name:" + obj.Name(),
		fmt.Sprintf("line:%d", span.StartLine),
		fmt.Sprintf("column:%d", span.StartColumn),
	}
	return strings.Join(parts, "|")
}

func isPackageLevel(obj types.Object, pkg *packages.Package) bool {
	return obj.Parent() != nil && pkg.Types != nil && obj.Parent() == pkg.Types.Scope()
}

func (e *emitter) referenceKind(ident *ast.Ident, obj types.Object) string {
	if _, ok := obj.(*types.PkgName); ok {
		return "package"
	}
	if _, ok := obj.(*types.TypeName); ok {
		return "type"
	}
	if e.isCallIdentifier(ident) {
		return "call"
	}
	return "read"
}

func (e *emitter) selectionReferenceKind(selector *ast.SelectorExpr, selection *types.Selection) string {
	if e.isCallSelector(selector) {
		return "call"
	}
	switch selection.Kind() {
	case types.FieldVal:
		return "field"
	case types.MethodVal, types.MethodExpr:
		return "method"
	default:
		return "member"
	}
}

func (e *emitter) isCallIdentifier(ident *ast.Ident) bool {
	parent := e.parents[ident]
	call, ok := parent.(*ast.CallExpr)
	return ok && call.Fun == ident
}

func (e *emitter) isCallSelector(selector *ast.SelectorExpr) bool {
	parent := e.parents[selector]
	call, ok := parent.(*ast.CallExpr)
	return ok && call.Fun == selector
}

func (e *emitter) selectorParent(ident *ast.Ident) *ast.SelectorExpr {
	selector, ok := e.parents[ident].(*ast.SelectorExpr)
	if !ok {
		return nil
	}
	return selector
}

func (e *emitter) addReference(packageID string, file string, name string, targetKey string, kind string, span Span) {
	key := strings.Join([]string{
		packageID,
		file,
		name,
		targetKey,
		kind,
		fmt.Sprintf("%d", span.StartByte),
		fmt.Sprintf("%d", span.EndByte),
	}, "|")
	e.refRows[key] = ReferenceRow{
		PackageID: packageID,
		File:      file,
		Name:      name,
		TargetKey: targetKey,
		Kind:      kind,
		Span:      span,
		Precision: "exact_semantic",
	}
}

func (e *emitter) ownerChainAt(pkg *packages.Package, pos token.Pos) []string {
	var ranges []ownerRange
	for _, owner := range e.ownerRanges {
		if owner.start <= pos && pos <= owner.end {
			ranges = append(ranges, owner)
		}
	}
	sort.Slice(ranges, func(i, j int) bool {
		if ranges[i].start == ranges[j].start {
			return ranges[i].end < ranges[j].end
		}
		return ranges[i].start < ranges[j].start
	})
	chain := make([]string, 0, len(ranges)+1)
	for _, owner := range ranges {
		chain = append(chain, owner.name)
	}
	if depth := scopeDepthAt(pkg, pos); depth > 0 {
		chain = append(chain, fmt.Sprintf("scope-depth:%d", depth))
	}
	return chain
}

func (e *emitter) finish() {
	for _, row := range e.symbolRows {
		e.out.Symbols = append(e.out.Symbols, row)
	}
	for _, row := range e.defRows {
		e.out.Definitions = append(e.out.Definitions, row)
	}
	for _, row := range e.refRows {
		e.out.References = append(e.out.References, row)
	}
	sort.Slice(e.out.Packages, func(i, j int) bool { return e.out.Packages[i].ID < e.out.Packages[j].ID })
	sort.Slice(e.out.Symbols, func(i, j int) bool { return e.out.Symbols[i].Key < e.out.Symbols[j].Key })
	sort.Slice(e.out.Definitions, func(i, j int) bool {
		return definitionOrderKey(e.out.Definitions[i]) < definitionOrderKey(e.out.Definitions[j])
	})
	sort.Slice(e.out.References, func(i, j int) bool {
		return referenceOrderKey(e.out.References[i]) < referenceOrderKey(e.out.References[j])
	})
	sort.Slice(e.out.Errors, func(i, j int) bool {
		left := e.out.Errors[i]
		right := e.out.Errors[j]
		return strings.Join([]string{left.PackageID, left.PackagePath, left.Message}, "\x00") <
			strings.Join([]string{right.PackageID, right.PackagePath, right.Message}, "\x00")
	})
}

func (e *emitter) identSpan(ident *ast.Ident) Span {
	if ident == nil {
		return Span{}
	}
	return e.span(ident.Pos(), ident.End())
}

func (e *emitter) span(start token.Pos, end token.Pos) Span {
	startPos := e.fset.PositionFor(start, false)
	endPos := e.fset.PositionFor(end, false)
	return Span{
		StartByte:   startPos.Offset,
		EndByte:     endPos.Offset,
		StartLine:   startPos.Line,
		StartColumn: startPos.Column,
		EndLine:     endPos.Line,
		EndColumn:   endPos.Column,
	}
}

func (e *emitter) fileForPos(pos token.Pos) string {
	filename := e.fset.PositionFor(pos, false).Filename
	return e.relativePath(filename)
}

func (e *emitter) relativePath(path string) string {
	if path == "" {
		return ""
	}
	abs, err := filepath.Abs(path)
	if err != nil {
		return filepath.ToSlash(path)
	}
	rel, err := filepath.Rel(e.root, abs)
	if err != nil || strings.HasPrefix(rel, "..") || filepath.IsAbs(rel) {
		return filepath.ToSlash(path)
	}
	return filepath.ToSlash(rel)
}

func safeObjectPath(obj types.Object) (path string, ok bool) {
	defer func() {
		if recover() != nil {
			path = ""
			ok = false
		}
	}()
	parts, err := objectpath.For(obj)
	if err != nil {
		return "", false
	}
	return string(parts), true
}

func scopeDepthAt(pkg *packages.Package, pos token.Pos) int {
	if pkg == nil {
		return 0
	}
	depth := 0
	for _, scope := range pkg.TypesInfo.Scopes {
		if scope == nil || scope.Pos() > pos || pos > scope.End() {
			continue
		}
		currentDepth := 0
		for current := scope; current != nil; current = current.Parent() {
			currentDepth++
		}
		if currentDepth > depth {
			depth = currentDepth
		}
	}
	return depth
}

func qualifiedName(obj types.Object) string {
	if obj == nil {
		return ""
	}
	if pkg := obj.Pkg(); pkg != nil && pkg.Path() != "" {
		return pkg.Path() + "." + obj.Name()
	}
	return obj.Name()
}

func modulePath(pkg *packages.Package) string {
	if pkg.Module == nil {
		return ""
	}
	return pkg.Module.Path
}

func testVariant(pkg *packages.Package) string {
	if strings.Contains(pkg.ID, ".test") || strings.Contains(pkg.ID, " [") || strings.HasSuffix(pkg.Name, "_test") {
		return "test"
	}
	return "regular"
}

func definitionKind(obj types.Object) string {
	switch obj.(type) {
	case *types.TypeName:
		return "type"
	case *types.PkgName:
		return "import"
	default:
		return "declaration"
	}
}

func receiverName(recv *ast.FieldList) string {
	if recv == nil || len(recv.List) == 0 {
		return ""
	}
	return exprName(recv.List[0].Type)
}

func exprName(expr ast.Expr) string {
	switch typed := expr.(type) {
	case *ast.Ident:
		return typed.Name
	case *ast.StarExpr:
		return exprName(typed.X)
	case *ast.SelectorExpr:
		return exprName(typed.X) + "." + typed.Sel.Name
	default:
		return ""
	}
}

func posLess(left token.Pos, right token.Pos, leftName string, rightName string) bool {
	if left == right {
		return leftName < rightName
	}
	return left < right
}

func definitionKey(symbolKey string, span Span, implicit bool) string {
	return strings.Join([]string{
		symbolKey,
		fmt.Sprintf("%t", implicit),
		fmt.Sprintf("%d", span.StartByte),
		fmt.Sprintf("%d", span.EndByte),
	}, "|")
}

func definitionOrderKey(row DefinitionRow) string {
	return strings.Join([]string{
		row.SymbolKey,
		row.File,
		fmt.Sprintf("%d", row.Span.StartByte),
		row.Name,
		row.Kind,
	}, "\x00")
}

func referenceOrderKey(row ReferenceRow) string {
	return strings.Join([]string{
		row.PackageID,
		row.File,
		fmt.Sprintf("%d", row.Span.StartByte),
		row.Name,
		row.Kind,
		row.TargetKey,
	}, "\x00")
}
