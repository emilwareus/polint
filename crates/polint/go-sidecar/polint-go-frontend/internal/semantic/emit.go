package semantic

import (
	"fmt"
	"go/ast"
	"go/token"
	"go/types"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"

	"golang.org/x/mod/modfile"
	"golang.org/x/tools/go/packages"
	"golang.org/x/tools/go/ssa"
	"golang.org/x/tools/go/ssa/ssautil"
)

const SchemaVersion = "polint-go-semantic-2"
const XToolsVersion = "v0.45.0"
const topologyManifestMaxBytes int64 = 1_048_576

type Config struct {
	Root         string
	ModuleRoots  []string
	Patterns     []string
	IncludeTests bool
	BuildTags    []string
}

type Row map[string]any

type Span struct {
	StartByte   int `json:"start_byte"`
	EndByte     int `json:"end_byte"`
	StartLine   int `json:"start_line"`
	StartColumn int `json:"start_column"`
	EndLine     int `json:"end_line"`
	EndColumn   int `json:"end_column"`
}

type emitter struct {
	root string
	fset *token.FileSet
	rows []Row
}

func Emit(config Config) ([]Row, error) {
	root, err := filepath.Abs(config.Root)
	if err != nil {
		return nil, fmt.Errorf("resolve root: %w", err)
	}
	moduleRoots, err := normalizeModuleRoots(config.ModuleRoots)
	if err != nil {
		return nil, err
	}
	if err := validateModuleRootFiles(root, moduleRoots); err != nil {
		return nil, err
	}
	patterns, err := validatePackagePatterns(config.Patterns)
	if err != nil {
		return nil, err
	}
	patterns = rootedPackagePatterns(moduleRoots, patterns)

	env, cleanup, err := goPackageEnv(root, moduleRoots)
	if err != nil {
		return nil, err
	}
	defer cleanup()

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
		Env:   env,
	}
	loadConfig.BuildFlags = goBuildFlags(config.BuildTags)

	pkgs, err := packages.Load(loadConfig, patterns...)
	if err != nil {
		return nil, err
	}
	sort.Slice(pkgs, func(i, j int) bool { return pkgs[i].ID < pkgs[j].ID })

	prog, ssaPkgs := ssautil.AllPackages(pkgs, ssa.SanityCheckFunctions)
	prog.Build()
	sort.Slice(ssaPkgs, func(i, j int) bool {
		return packageID(ssaPkgs[i]) < packageID(ssaPkgs[j])
	})

	e := &emitter{root: root, fset: fset}
	e.add(Row{
		"kind":            "session_begin",
		"schema":          SchemaVersion,
		"go_version":      runtime.Version(),
		"x_tools_version": XToolsVersion,
	})
	for _, pkg := range pkgs {
		e.emitPackage(pkg)
		e.emitPackageErrors(pkg)
	}
	for _, pkg := range ssaPkgs {
		e.emitSSAPackage(pkg)
	}
	e.add(Row{
		"kind":       "session_end",
		"schema":     SchemaVersion,
		"row_count":  len(e.rows) + 1,
		"go_version": runtime.Version(),
	})
	return e.rows, nil
}

func (e *emitter) add(row Row) {
	if _, ok := row["schema"]; !ok {
		row["schema"] = SchemaVersion
	}
	e.rows = append(e.rows, row)
}

func (e *emitter) emitPackage(pkg *packages.Package) {
	modulePath := ""
	if pkg.Module != nil {
		modulePath = pkg.Module.Path
	}
	files := append([]string{}, pkg.CompiledGoFiles...)
	sort.Strings(files)
	for i, file := range files {
		files[i] = e.relative(file)
	}
	e.add(Row{
		"kind":         "package",
		"package_id":   pkg.ID,
		"package_path": pkg.PkgPath,
		"package_name": pkg.Name,
		"module_path":  modulePath,
		"test_variant": testVariant(pkg.ID),
		"files":        files,
		"stable_key":   stableKey("package", pkg.ID, pkg.PkgPath),
	})
}

func (e *emitter) emitPackageErrors(pkg *packages.Package) {
	for index, err := range pkg.Errors {
		e.add(Row{
			"kind":         "package_error",
			"package_id":   pkg.ID,
			"package_path": pkg.PkgPath,
			"message":      err.Msg,
			"stable_key":   stableKey("package_error", pkg.ID, strconv.Itoa(index), err.Msg),
		})
	}
}

func (e *emitter) emitSSAPackage(pkg *ssa.Package) {
	if pkg == nil || pkg.Pkg == nil {
		return
	}
	functions := ssaFunctions(pkg)
	for _, fn := range functions {
		e.emitFunction(pkg, fn)
		e.emitCallsites(pkg, fn)
		e.emitInstantiatedTypes(pkg, fn)
		e.emitAddressTaken(pkg, fn)
	}
	e.emitMethodSets(pkg)
}

func (e *emitter) emitFunction(pkg *ssa.Package, fn *ssa.Function) {
	if fn == nil {
		return
	}
	isInit := isInitFunctionName(fn.Name())
	isMethod := fn.Signature != nil && fn.Signature.Recv() != nil
	if fn.Synthetic != "" && !isInit && !isMethod && !fn.Pos().IsValid() {
		e.add(Row{
			"kind":         "unsupported",
			"package_id":   packageID(pkg),
			"package_path": packagePath(pkg),
			"name":         fn.String(),
			"reason":       "synthetic function without stable source identity",
		})
		return
	}
	kind := "function"
	if isInit {
		kind = "init_function"
	} else if isMethod {
		kind = "method"
	}
	name := fn.Name()
	if isInit {
		name = "init"
	} else if isMethod && fn.Signature != nil && fn.Signature.Recv() != nil {
		if receiver := receiverTypeName(fn.Signature.Recv().Type().String()); receiver != "" {
			name = receiver + "." + fn.Name()
		}
	}
	row := Row{
		"kind":         kind,
		"package_id":   packageID(pkg),
		"package_path": packagePath(pkg),
		"name":         name,
		"qualified":    fn.String(),
		"signature":    signatureString(fn.Signature),
		"stable_key":   stableKey(packageID(pkg), fn.String()),
	}
	if syntax := fn.Syntax(); syntax != nil {
		if pos := e.positionSpan(syntax.Pos(), syntax.End()); pos != nil {
			row["file"] = posFile(e.fset, syntax.Pos(), e.root)
			row["span"] = pos
		}
	} else if pos := e.positionSpan(fn.Pos(), fn.Pos()); pos != nil {
		row["file"] = posFile(e.fset, fn.Pos(), e.root)
		row["span"] = pos
	}
	if fn.Signature != nil && fn.Signature.Recv() != nil {
		row["receiver"] = fn.Signature.Recv().Type().String()
		e.add(Row{
			"kind":         "receiver_type",
			"package_id":   packageID(pkg),
			"package_path": packagePath(pkg),
			"method":       fn.String(),
			"receiver":     fn.Signature.Recv().Type().String(),
			"stable_key":   stableKey(packageID(pkg), "recv", fn.String(), fn.Signature.Recv().Type().String()),
		})
	}
	e.add(row)
}

func (e *emitter) emitCallsites(pkg *ssa.Package, fn *ssa.Function) {
	if fn == nil {
		return
	}
	for _, block := range fn.Blocks {
		for _, instr := range block.Instrs {
			call, ok := instr.(ssa.CallInstruction)
			if !ok {
				continue
			}
			common := call.Common()
			row := Row{
				"kind":         "callsite",
				"package_id":   packageID(pkg),
				"package_path": packagePath(pkg),
				"caller":       fn.String(),
			}
			dynamic := false
			if common != nil && common.StaticCallee() != nil {
				row["static_callee"] = common.StaticCallee().String()
				row["status"] = "resolved_static"
			} else {
				row["status"] = "unresolved_dynamic"
				row["reason"] = "interface or func-value dynamic dispatch"
				dynamic = true
			}
			stableParts := []string{packageID(pkg), fn.String(), fmt.Sprint(call.Pos())}
			if syntax := callSyntax(fn, call); syntax != nil {
				if pos := e.positionSpan(syntax.Pos(), syntax.End()); pos != nil {
					file := posFile(e.fset, syntax.Pos(), e.root)
					row["file"] = file
					row["span"] = pos
					stableParts = []string{
						packageID(pkg),
						fn.String(),
						file,
						strconv.Itoa(pos.StartByte),
						strconv.Itoa(pos.EndByte),
					}
				}
			} else if pos := e.positionSpan(call.Pos(), call.Pos()); pos != nil {
				row["file"] = posFile(e.fset, call.Pos(), e.root)
				row["span"] = pos
			}
			callsiteKey := stableKey(stableParts...)
			row["stable_key"] = callsiteKey
			e.add(row)
			if dynamic {
				e.emitDynamicDispatch(pkg, fn, common, callsiteKey)
			}
		}
	}
}

// emitDynamicDispatch emits the dispatch discriminant Plan 2's RTA driver needs to
// resolve an UnresolvedDynamic callsite by method-set matching (D-05). For an interface
// invoke it carries the interface type + invoked method name; for a func-value call it
// carries the called value's signature. The row joins back to its sibling callsite row
// via callsite_stable_key. Honest representation (Phase 46 D-15): if no discriminant can
// be derived, no dispatch-detail row is emitted rather than a fabricated identity.
func (e *emitter) emitDynamicDispatch(pkg *ssa.Package, fn *ssa.Function, common *ssa.CallCommon, callsiteKey string) {
	if common == nil {
		return
	}
	row := Row{
		"kind":                "dynamic_dispatch",
		"package_id":          packageID(pkg),
		"package_path":        packagePath(pkg),
		"caller":              fn.String(),
		"callsite_stable_key": callsiteKey,
	}
	var discriminant string
	if common.IsInvoke() && common.Method != nil {
		interfaceType := ""
		if common.Value != nil && common.Value.Type() != nil {
			interfaceType = common.Value.Type().String()
		}
		method := common.Method.Name()
		row["interface_type"] = interfaceType
		row["method"] = method
		discriminant = "invoke:" + interfaceType + ":" + method
	} else {
		signature := ""
		if sig := common.Signature(); sig != nil {
			signature = sig.String()
		}
		if signature == "" {
			return
		}
		row["signature"] = signature
		discriminant = "func_value:" + signature
	}
	row["stable_key"] = stableKey(packageID(pkg), "dynamic_dispatch", callsiteKey, discriminant)
	e.add(row)
}

// emitInstantiatedTypes harvests the RTA "rapid type" set: each concrete type converted
// to an interface via *ssa.MakeInterface in the reachable SSA program (D-05). x/tools RTA
// adds a type to the rapid-type set precisely when it is converted to an interface, so
// MakeInterface is the faithful and sufficient source. We deliberately do NOT harvest the
// *ssa.Alloc / *ssa.MakeMap / *ssa.MakeSlice / *ssa.MakeChan families: allocating a value
// does not by itself make its type dynamically dispatchable under RTA — only an
// interface conversion does — so adding them would over-approximate the rapid-type set and
// flood precision without lifting recall. Types lacking a stable .String() identity emit an
// "unsupported" row rather than a fabricated identity (Phase 46 D-15). De-duplicated within
// a function by the concrete type identity.
func (e *emitter) emitInstantiatedTypes(pkg *ssa.Package, fn *ssa.Function) {
	if fn == nil {
		return
	}
	seen := make(map[string]bool)
	for _, block := range fn.Blocks {
		for _, instr := range block.Instrs {
			mi, ok := instr.(*ssa.MakeInterface)
			if !ok {
				continue
			}
			if mi.X == nil || mi.X.Type() == nil {
				e.add(Row{
					"kind":         "unsupported",
					"package_id":   packageID(pkg),
					"package_path": packagePath(pkg),
					"name":         fn.String(),
					"reason":       "MakeInterface operand without stable type identity",
				})
				continue
			}
			concrete := mi.X.Type().String()
			if concrete == "" || seen[concrete] {
				continue
			}
			seen[concrete] = true
			e.add(Row{
				"kind":         "instantiated_type",
				"package_id":   packageID(pkg),
				"package_path": packagePath(pkg),
				"type":         concrete,
				"stable_key":   stableKey(packageID(pkg), "instantiated_type", concrete),
			})
		}
	}
}

// emitAddressTaken harvests the set of functions whose address is taken in the reachable
// SSA program (D-05) — an RTA dispatch input for func-value callsites. Sources: *ssa.MakeClosure
// over an *ssa.Function (closures and bound method values), and any *ssa.Function used as a
// value operand of an instruction (function references passed as args, stored to globals, or
// assigned). De-duplicated within a function by the function identity; builtins/synthetic
// functions without stable identity are skipped rather than fabricated (Phase 46 D-15).
func (e *emitter) emitAddressTaken(pkg *ssa.Package, fn *ssa.Function) {
	if fn == nil {
		return
	}
	seen := make(map[string]bool)
	emit := func(target *ssa.Function) {
		if target == nil {
			return
		}
		identity := target.String()
		if identity == "" || seen[identity] {
			return
		}
		seen[identity] = true
		e.add(Row{
			"kind":         "address_taken",
			"package_id":   packageID(pkg),
			"package_path": packagePath(pkg),
			"function":     identity,
			"stable_key":   stableKey(packageID(pkg), "address_taken", identity),
		})
	}
	for _, block := range fn.Blocks {
		for _, instr := range block.Instrs {
			if closure, ok := instr.(*ssa.MakeClosure); ok {
				if target, ok := closure.Fn.(*ssa.Function); ok {
					emit(target)
				}
			}
			operands := instr.Operands(nil)
			for _, operand := range operands {
				if operand == nil || *operand == nil {
					continue
				}
				if target, ok := (*operand).(*ssa.Function); ok {
					emit(target)
				}
			}
		}
	}
}

func callSyntax(fn *ssa.Function, call ssa.CallInstruction) ast.Node {
	if fn == nil || !call.Pos().IsValid() {
		return nil
	}
	syntax := fn.Syntax()
	if syntax == nil {
		return nil
	}
	pos := call.Pos()
	var best ast.Node
	ast.Inspect(syntax, func(node ast.Node) bool {
		if node == nil {
			return true
		}
		expr, ok := node.(*ast.CallExpr)
		if !ok {
			return true
		}
		if expr.Pos() <= pos && pos < expr.End() {
			if best == nil || expr.End()-expr.Pos() < best.End()-best.Pos() {
				best = expr
			}
		}
		return true
	})
	return best
}

func (e *emitter) emitMethodSets(pkg *ssa.Package) {
	scope := pkg.Pkg.Scope()
	for _, name := range scope.Names() {
		obj := scope.Lookup(name)
		typeName, ok := obj.(*types.TypeName)
		if !ok {
			continue
		}
		methodSet := types.NewMethodSet(types.NewPointer(typeName.Type()))
		if methodSet.Len() == 0 {
			continue
		}
		var methods []string
		for i := 0; i < methodSet.Len(); i++ {
			// The method-set carries the bare method NAME (the identifier, e.g.
			// "Speak"), not the full signature string. RTA interface-invoke
			// resolution intersects the INVOKED method name (the dynamic-dispatch
			// discriminant) with this set, so a signature string would never match
			// the invoked name and interface dispatch would resolve nothing (Phase 48
			// verification surfaced this). `Obj().Name()` is the method identifier.
			methods = append(methods, methodSet.At(i).Obj().Name())
		}
		sort.Strings(methods)
		e.add(Row{
			"kind":         "method_set",
			"package_id":   packageID(pkg),
			"package_path": packagePath(pkg),
			"type":         typeName.Type().String(),
			"methods":      methods,
			"stable_key":   stableKey(packageID(pkg), "method_set", typeName.Type().String()),
		})
	}
}

func ssaFunctions(pkg *ssa.Package) []*ssa.Function {
	var functions []*ssa.Function
	// Dedup by *ssa.Function identity so a function reachable via two roots (e.g. a
	// method value that is also harvested as a top-level member) is walked once.
	seen := make(map[*ssa.Function]bool)
	for _, member := range pkg.Members {
		switch value := member.(type) {
		case *ssa.Function:
			collectWithAnon(value, seen, &functions)
		case *ssa.Type:
			methodSet := pkg.Prog.MethodSets.MethodSet(types.NewPointer(value.Type()))
			for i := 0; i < methodSet.Len(); i++ {
				if fn := pkg.Prog.MethodValue(methodSet.At(i)); fn != nil {
					collectWithAnon(fn, seen, &functions)
				}
			}
		}
	}
	sort.Slice(functions, func(i, j int) bool { return functions[i].String() < functions[j].String() })
	return functions
}

// collectWithAnon appends fn and, transitively, its anonymous-function bodies
// (closures, func(){...} literals, bound method-value thunks) to out. In go/ssa
// these live in parent.AnonFuncs and are NOT among pkg.Members, so without this walk
// a *ssa.MakeInterface, a dynamic callsite, or a function-value operand that appears
// inside a closure body would be invisible to the RTA harvest (review WR-01). Closures
// nest, so the walk is transitive; `seen` guards against double-visiting.
func collectWithAnon(fn *ssa.Function, seen map[*ssa.Function]bool, out *[]*ssa.Function) {
	if fn == nil || seen[fn] {
		return
	}
	seen[fn] = true
	*out = append(*out, fn)
	for _, anon := range fn.AnonFuncs {
		collectWithAnon(anon, seen, out)
	}
}

func packageID(pkg *ssa.Package) string {
	if pkg == nil || pkg.Pkg == nil {
		return ""
	}
	return pkg.Pkg.Path()
}

func packagePath(pkg *ssa.Package) string {
	return packageID(pkg)
}

func signatureString(sig *types.Signature) string {
	if sig == nil {
		return ""
	}
	return sig.String()
}

func receiverTypeName(receiver string) string {
	receiver = strings.TrimSpace(receiver)
	receiver = strings.TrimLeft(receiver, "*&")
	if receiver == "" {
		return ""
	}
	receiver = receiver[strings.LastIndex(receiver, ".")+1:]
	if index := strings.Index(receiver, "["); index >= 0 {
		receiver = receiver[:index]
	}
	return receiver
}

func isInitFunctionName(name string) bool {
	return name == "init" || strings.HasPrefix(name, "init#") || strings.HasPrefix(name, "init$")
}

func stableKey(parts ...string) string {
	var builder strings.Builder
	for _, part := range parts {
		builder.WriteString(strconv.Itoa(len(part)))
		builder.WriteString(":")
		builder.WriteString(part)
		builder.WriteString(";")
	}
	return builder.String()
}

func (e *emitter) positionSpan(start token.Pos, end token.Pos) *Span {
	if !start.IsValid() {
		return nil
	}
	startPos := e.fset.Position(start)
	endPos := e.fset.Position(end)
	return &Span{
		StartByte:   startPos.Offset,
		EndByte:     endPos.Offset,
		StartLine:   startPos.Line,
		StartColumn: startPos.Column,
		EndLine:     endPos.Line,
		EndColumn:   endPos.Column,
	}
}

func posFile(fset *token.FileSet, pos token.Pos, root string) string {
	position := fset.Position(pos)
	if position.Filename == "" {
		return ""
	}
	if rel, err := filepath.Rel(root, position.Filename); err == nil {
		return filepath.ToSlash(rel)
	}
	return filepath.ToSlash(position.Filename)
}

func (e *emitter) relative(path string) string {
	if rel, err := filepath.Rel(e.root, path); err == nil {
		return filepath.ToSlash(rel)
	}
	return filepath.ToSlash(path)
}

func testVariant(id string) string {
	switch {
	case strings.HasSuffix(id, ".test"):
		return "test_binary"
	case strings.Contains(id, " ["):
		return "external_test"
	default:
		return "regular"
	}
}

func goBuildFlags(buildTags []string) []string {
	flags := []string{"-mod=readonly"}
	if len(buildTags) > 0 {
		flags = append(flags, "-tags="+strings.Join(buildTags, ","))
	}
	return flags
}

func validatePackagePatterns(patterns []string) ([]string, error) {
	var normalized []string
	for _, raw := range patterns {
		pattern := strings.TrimSpace(raw)
		if pattern == "" {
			continue
		}
		if strings.HasPrefix(pattern, "-") {
			return nil, fmt.Errorf("package pattern %q must not start with -", pattern)
		}
		normalized = append(normalized, pattern)
	}
	if len(normalized) == 0 {
		return []string{"./..."}, nil
	}
	return normalized, nil
}

func validateModuleRootFiles(root string, moduleRoots []string) error {
	for _, moduleRoot := range moduleRoots {
		if !repoRegularFileExists(root, moduleRootManifest(moduleRoot)) {
			return fmt.Errorf("module root %q is missing an in-repository go.mod", moduleRoot)
		}
	}
	return nil
}

func moduleRootManifest(moduleRoot string) string {
	if moduleRoot == "." {
		return "go.mod"
	}
	return filepath.ToSlash(filepath.Join(filepath.FromSlash(moduleRoot), "go.mod"))
}

func normalizeModuleRoots(roots []string) ([]string, error) {
	if len(roots) == 0 {
		roots = []string{"."}
	}
	seen := make(map[string]bool)
	var normalized []string
	for _, raw := range roots {
		root := strings.TrimSpace(raw)
		if root == "" || root == "." {
			root = "."
		}
		if filepath.IsAbs(root) {
			return nil, fmt.Errorf("module root %q must be relative to repository root", raw)
		}
		clean := filepath.Clean(filepath.FromSlash(root))
		if clean == "." {
			root = "."
		} else {
			if clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
				return nil, fmt.Errorf("module root %q escapes repository root", raw)
			}
			root = filepath.ToSlash(clean)
		}
		if !seen[root] {
			normalized = append(normalized, root)
			seen[root] = true
		}
	}
	return normalized, nil
}

func rootedPackagePatterns(moduleRoots []string, patterns []string) []string {
	seen := make(map[string]bool)
	var rooted []string
	for _, root := range moduleRoots {
		for _, pattern := range patterns {
			rootedPattern := rootedPackagePattern(root, pattern)
			if !seen[rootedPattern] {
				rooted = append(rooted, rootedPattern)
				seen[rootedPattern] = true
			}
		}
	}
	return rooted
}

func rootedPackagePattern(root string, pattern string) string {
	if root == "." {
		return pattern
	}
	base := "./" + strings.TrimPrefix(root, "./")
	if pattern == "." {
		return base
	}
	if strings.HasPrefix(pattern, "./") {
		suffix := strings.TrimPrefix(pattern, "./")
		if suffix == "" {
			return base
		}
		return base + "/" + suffix
	}
	return pattern
}

func goPackageEnv(root string, moduleRoots []string) ([]string, func(), error) {
	env := os.Environ()
	workspace, cleanup, err := workspaceEnv(root, moduleRoots)
	if err != nil {
		return nil, nil, err
	}
	env = setEnv(env, "GOWORK", workspace)
	return env, cleanup, nil
}

func setEnv(env []string, key string, value string) []string {
	prefix := key + "="
	filtered := make([]string, 0, len(env)+1)
	for _, entry := range env {
		if !strings.HasPrefix(entry, prefix) {
			filtered = append(filtered, entry)
		}
	}
	return append(filtered, prefix+value)
}

func workspaceEnv(root string, moduleRoots []string) (string, func(), error) {
	checkedIn := filepath.Join(root, "go.work")
	if repoRegularFileExists(root, "go.work") && goWorkCoversModuleRoots(root, checkedIn, moduleRoots) {
		return checkedIn, func() {}, nil
	}
	if !needsSyntheticWorkspace(moduleRoots) {
		return "off", func() {}, nil
	}
	return writeSyntheticGoWork(root, moduleRoots)
}

func needsSyntheticWorkspace(moduleRoots []string) bool {
	return len(moduleRoots) != 1 || moduleRoots[0] != "."
}

func goWorkCoversModuleRoots(root string, workPath string, moduleRoots []string) bool {
	contents, err := readFileWithLimit(workPath, topologyManifestMaxBytes)
	if err != nil {
		return false
	}
	work, err := modfile.ParseWork(workPath, contents, nil)
	if err != nil {
		return false
	}
	rootReal, err := realCleanPath(root)
	if err != nil {
		return false
	}
	covered := make(map[string]bool)
	for _, use := range work.Use {
		usePath := filepath.Clean(filepath.FromSlash(use.Path))
		if !filepath.IsAbs(usePath) {
			usePath = filepath.Join(root, usePath)
		}
		realUse, err := realCleanPath(usePath)
		if err != nil || !isUnderRoot(rootReal, realUse) {
			continue
		}
		covered[realUse] = true
	}
	for _, moduleRoot := range moduleRoots {
		modulePath := filepath.Join(root, filepath.FromSlash(moduleRoot))
		realModule, err := realCleanPath(modulePath)
		if err != nil || !covered[realModule] {
			return false
		}
	}
	return true
}

func writeSyntheticGoWork(root string, moduleRoots []string) (string, func(), error) {
	dir, err := os.MkdirTemp("", "polint-go-work-")
	if err != nil {
		return "", nil, fmt.Errorf("create temporary go.work directory: %w", err)
	}
	cleanup := func() { _ = os.RemoveAll(dir) }
	workPath := filepath.Join(dir, "go.work")
	var contents strings.Builder
	contents.WriteString("go ")
	contents.WriteString(syntheticGoWorkVersion(root, moduleRoots))
	contents.WriteString("\n\nuse (\n")
	for _, moduleRoot := range moduleRoots {
		modulePath := filepath.Join(root, filepath.FromSlash(moduleRoot))
		contents.WriteString("\t")
		contents.WriteString(strconv.Quote(filepath.ToSlash(goWorkUsePath(dir, modulePath))))
		contents.WriteString("\n")
	}
	contents.WriteString(")\n")
	if err := os.WriteFile(workPath, []byte(contents.String()), 0o600); err != nil {
		cleanup()
		return "", nil, fmt.Errorf("write temporary go.work: %w", err)
	}
	return workPath, cleanup, nil
}

func syntheticGoWorkVersion(root string, moduleRoots []string) string {
	version := "1.24"
	for _, moduleRoot := range moduleRoots {
		modPath := filepath.Join(root, filepath.FromSlash(moduleRoot), "go.mod")
		contents, err := readFileWithLimit(modPath, topologyManifestMaxBytes)
		if err != nil {
			continue
		}
		parsed, err := modfile.Parse(modPath, contents, nil)
		if err != nil || parsed.Go == nil {
			continue
		}
		if compareGoVersion(parsed.Go.Version, version) > 0 {
			version = parsed.Go.Version
		}
	}
	return version
}

func goWorkUsePath(workDir string, modulePath string) string {
	rel, err := filepath.Rel(workDir, modulePath)
	if err != nil || rel == "" {
		return modulePath
	}
	return rel
}

func compareGoVersion(left string, right string) int {
	leftParts := versionInts(left)
	rightParts := versionInts(right)
	for i := 0; i < len(leftParts) || i < len(rightParts); i++ {
		var l, r int
		if i < len(leftParts) {
			l = leftParts[i]
		}
		if i < len(rightParts) {
			r = rightParts[i]
		}
		if l < r {
			return -1
		}
		if l > r {
			return 1
		}
	}
	return 0
}

func versionInts(version string) []int {
	var values []int
	for _, part := range strings.Split(strings.TrimPrefix(version, "go"), ".") {
		value, err := strconv.Atoi(part)
		if err != nil {
			values = append(values, 0)
			continue
		}
		values = append(values, value)
	}
	return values
}

func readFileWithLimit(path string, maxBytes int64) ([]byte, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	contents, err := io.ReadAll(io.LimitReader(file, maxBytes+1))
	if err != nil {
		return nil, err
	}
	if int64(len(contents)) > maxBytes {
		return nil, fmt.Errorf("%s exceeds %d bytes", path, maxBytes)
	}
	return contents, nil
}

func realCleanPath(path string) (string, error) {
	abs, err := filepath.Abs(path)
	if err != nil {
		return "", err
	}
	real, err := filepath.EvalSymlinks(abs)
	if err != nil {
		return "", err
	}
	return filepath.Clean(real), nil
}

func isUnderRoot(root string, path string) bool {
	rel, err := filepath.Rel(root, path)
	if err != nil {
		return false
	}
	return rel == "." || (rel != ".." && !strings.HasPrefix(filepath.ToSlash(rel), "../"))
}

func repoRegularFileExists(root string, relative string) bool {
	path := filepath.Join(root, filepath.FromSlash(relative))
	rel, err := filepath.Rel(root, path)
	if err != nil || strings.HasPrefix(filepath.ToSlash(rel), "../") {
		return false
	}
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular()
}
