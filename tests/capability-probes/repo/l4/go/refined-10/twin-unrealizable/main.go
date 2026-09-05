package twinunrealizable

type holderGo10 struct{}

func (h holderGo10) seedTargetGo10() {}
func seedCallerGo10() {
	if false { holderGo10{}.seedTargetGo10() }
}
