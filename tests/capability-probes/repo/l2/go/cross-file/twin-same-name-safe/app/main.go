package app

import "example.com/capability-probes/l2/go/cross-file/twin-same-name-safe/safe"

func Probe() { safe.ForbiddenCross() }
