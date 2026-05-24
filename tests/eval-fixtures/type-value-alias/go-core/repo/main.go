package main

type Service struct {
	Name string
}

func Rename(service *Service, name string) string {
	service.Name = name
	again := service
	return again.Name
}
