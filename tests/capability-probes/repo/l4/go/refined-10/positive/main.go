package positive

type holderGo10 struct{}

func (h holderGo10) seedTargetGo10() {}
func seedCallerGo10() { holderGo10{}.seedTargetGo10() }
