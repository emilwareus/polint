package twinmayedge

func seedTargetGo09() {}
func seedCallerGo09() {
	chosen := seedTargetGo09
	chosen()
}

