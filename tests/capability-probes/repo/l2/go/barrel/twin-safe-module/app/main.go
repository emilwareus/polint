package app

import "example.com/capability-probes/l2/go/barrel/twin-safe-module/facade"

func Probe() { facade.ForbiddenBarrel() }
