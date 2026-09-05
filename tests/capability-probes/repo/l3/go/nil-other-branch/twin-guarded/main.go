package twinguarded

type Item struct{ Value string }
func useNil(value string) {}
func Probe(input *Item) {
	if input == nil { return }
	useNil(input.Value)
}
