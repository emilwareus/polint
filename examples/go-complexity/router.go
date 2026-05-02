package router

func Route(kind string, ready bool) string {
	if kind == "admin" {
		return "admin"
	}
	if ready {
		return "ready"
	}
	return "guest"
}
