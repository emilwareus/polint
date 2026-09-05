package twinmayedge

type holderGo10 struct{}

func (h holderGo10) seedTargetGo10() {}
func seedCallerGo10() {
	chosen := holderGo10{}.seedTargetGo10
	chosen()
}
