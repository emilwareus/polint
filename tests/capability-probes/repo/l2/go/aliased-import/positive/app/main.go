package app

import legacy "example.com/capability-probes/l2/go/aliased-import/positive/bad"

func Probe() { legacy.ForbiddenAlias() }
