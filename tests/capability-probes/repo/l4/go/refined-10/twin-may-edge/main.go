package twinmayedge

func seedTargetGo10() {}
func seedCallerGo10() {
	chosen := seedTargetGo10
	chosen()
}

