package app

import legacy "example.com/capability-probes/l2/go/aliased-import/twin-safe-module/safe"

func Probe() { legacy.ForbiddenAlias() }
