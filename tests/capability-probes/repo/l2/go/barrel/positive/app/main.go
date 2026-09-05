package app

import "example.com/capability-probes/l2/go/barrel/positive/facade"

func Probe() { facade.ForbiddenBarrel() }
