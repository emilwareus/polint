package symbols

import (
	"reflect"
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
