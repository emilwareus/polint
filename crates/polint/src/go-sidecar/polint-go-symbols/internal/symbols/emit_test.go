package symbols

import (
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"testing"
)

func TestGoBuildFlagsAlwaysUsesReadonlyModules(t *testing.T) {
	got := goBuildFlags(nil)
	want := []string{"-mod=readonly"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("goBuildFlags(nil) = %#v, want %#v", got, want)
	}
}

func TestGoBuildFlagsKeepsBuildTags(t *testing.T) {
	got := goBuildFlags([]string{"integration", "linux"})
	want := []string{"-mod=readonly", "-tags=integration,linux"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("goBuildFlags(tags) = %#v, want %#v", got, want)
	}
}

func TestValidatePackagePatternsRejectsFlags(t *testing.T) {
	_, err := validatePackagePatterns([]string{"./...", "-json"})
	if err == nil {
		t.Fatalf("validatePackagePatterns accepted a go list flag")
	}
}

func TestValidateModuleRootFilesRejectsSymlinkAncestor(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.MkdirAll(filepath.Join(outside, "app"), 0o755); err != nil {
		t.Fatalf("mkdir outside app: %v", err)
	}
	if err := os.WriteFile(filepath.Join(outside, "app", "go.mod"), []byte("module example.com/app\n\ngo 1.24\n"), 0o644); err != nil {
		t.Fatalf("write outside go.mod: %v", err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "link")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	if err := validateModuleRootFiles(root, []string{"link/app"}); err == nil {
		t.Fatalf("validateModuleRootFiles accepted symlinked module root")
	}
}

func TestGoWorkCoversModuleRootsRejectsOutsideUse(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "app"), 0o755); err != nil {
		t.Fatalf("mkdir app: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "app", "go.mod"), []byte("module example.com/app\n\ngo 1.24\n"), 0o644); err != nil {
		t.Fatalf("write app go.mod: %v", err)
	}
	workPath := filepath.Join(root, "go.work")
	contents := "go 1.24.0\n\nuse (\n\t./app\n\t" + strconv.Quote(filepath.ToSlash(outside)) + "\n)\n"
	if err := os.WriteFile(workPath, []byte(contents), 0o644); err != nil {
		t.Fatalf("write go.work: %v", err)
	}

	if goWorkCoversModuleRoots(root, []string{"app"}) {
		t.Fatalf("goWorkCoversModuleRoots accepted go.work with outside use entry")
	}
}

func TestGoWorkCoversModuleRootsRejectsSymlinkUse(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "app"), 0o755); err != nil {
		t.Fatalf("mkdir app: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "app", "go.mod"), []byte("module example.com/app\n\ngo 1.24\n"), 0o644); err != nil {
		t.Fatalf("write app go.mod: %v", err)
	}
	if err := os.MkdirAll(filepath.Join(outside, "link"), 0o755); err != nil {
		t.Fatalf("mkdir outside link: %v", err)
	}
	if err := os.WriteFile(filepath.Join(outside, "link", "go.mod"), []byte("module example.com/link\n\ngo 1.24\n"), 0o644); err != nil {
		t.Fatalf("write outside go.mod: %v", err)
	}
	if err := os.Symlink(filepath.Join(outside, "link"), filepath.Join(root, "link")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	workPath := filepath.Join(root, "go.work")
	contents := "go 1.24.0\n\nuse (\n\t./app\n\t./link\n)\n"
	if err := os.WriteFile(workPath, []byte(contents), 0o644); err != nil {
		t.Fatalf("write go.work: %v", err)
	}

	if goWorkCoversModuleRoots(root, []string{"app"}) {
		t.Fatalf("goWorkCoversModuleRoots accepted symlinked go.work use entry")
	}
}

func TestGoPackageEnvRejectsGoWorkSymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	outsideWork := filepath.Join(outside, "go.work")
	if err := os.WriteFile(outsideWork, []byte("go 1.24.0\n\nuse ./app\n"), 0o644); err != nil {
		t.Fatalf("write outside go.work: %v", err)
	}
	if err := os.Symlink(outsideWork, filepath.Join(root, "go.work")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	env, cleanup, err := goPackageEnv(root, []string{"app"})
	if err != nil {
		t.Fatalf("goPackageEnv: %v", err)
	}
	defer cleanup()

	for _, value := range env {
		if value == "GOWORK="+filepath.Join(root, "go.work") {
			t.Fatalf("goPackageEnv reused symlinked go.work")
		}
	}
}
