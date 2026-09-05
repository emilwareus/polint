package main

func dangerousExec(command string) {}

func main() {
	dangerousExec("rm")
}
