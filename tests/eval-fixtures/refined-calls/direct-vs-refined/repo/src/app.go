package main

type Speaker interface {
	Speak() string
}

type Dog struct{}

func (Dog) Speak() string {
	return "woof"
}

func runDispatch() {
	var speaker Speaker = Dog{}
	speaker.Speak()
}

func main() {
	runDispatch()
}
