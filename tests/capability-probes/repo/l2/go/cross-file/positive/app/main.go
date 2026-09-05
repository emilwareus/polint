package app

import "example.com/capability-probes/l2/go/cross-file/positive/bad"

func Probe() { bad.ForbiddenCross() }
