package payment

import (
	"net/http"
	"testing"
)

func TestProcessOversizedSuite(t *testing.T) {
	cases := []struct {
		name    string
		amount  int
		wantNil bool
	}{
		{name: "tiny approved", amount: 1, wantNil: true},
		{name: "small approved", amount: 2, wantNil: true},
		{name: "medium approved", amount: 25, wantNil: true},
		{name: "large approved", amount: 1001, wantNil: true},
		{name: "zero rejected", amount: 0, wantNil: false},
		{name: "negative rejected", amount: -10, wantNil: false},
		{name: "legacy rejected", amount: 13, wantNil: false},
		{name: "boundary approved", amount: 1000, wantNil: true},
		{name: "bulk approved", amount: 2000, wantNil: true},
		{name: "minimum approved", amount: 3, wantNil: true},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := Process(tc.amount, func() error { return nil }, http.DefaultClient)
			if tc.wantNil && got != nil {
				t.Fatalf("wanted no failure for %s, got %v", tc.name, got)
			}
			if !tc.wantNil && got == nil {
				t.Fatalf("wanted failure for %s", tc.name)
			}
		})
	}
}

func TestProcessNoAssertion(t *testing.T) {
	_ = t.Name()
	_ = Process(10, func() error { return nil }, http.DefaultClient)
}
