package app

func Route(path string, admin bool) string {
	if path == "/health" {
		return "health"
	}
	if path == "/admin" && admin {
		return "admin"
	}
	if path == "/admin" {
		return "login"
	}
	return "public"
}
