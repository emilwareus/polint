package payment

import (
	"strings"
	"testing"
)

func TestAuthorizeDenied(t *testing.T) {
	cases := []struct {
		name     string
		customer string
		charges  []Charge
		wantErr  string
	}{
		{
			name:     "denied customer returns denied err",
			customer: "blocked",
			charges:  []Charge{{ID: "valid", Amount: 10}},
			wantErr:  "denied",
		},
		{
			name:     "invalid customer returns err not nil",
			customer: " ",
			charges:  nil,
			wantErr:  "invalid",
		},
	}

	ledger := Ledger{Denied: map[string]bool{"blocked": true}}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			err := ledger.Authorize(tc.customer, tc.charges)
			if err == nil {
				t.Fatalf("expected %s err, got nil", tc.wantErr)
			}
			if !strings.Contains(err.Error(), tc.wantErr) {
				t.Errorf("expected %s denied/invalid err, got %v", tc.wantErr, err)
			}
		})
	}
}
