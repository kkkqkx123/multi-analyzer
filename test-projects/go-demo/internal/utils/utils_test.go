package utils

import (
	"testing"
	"time"
)

func TestAdd(t *testing.T) {
	if Add(1, 2) != 3 {
		t.Fatal("Add(1,2) should be 3")
	}
}

// TestFormatDate deliberately fails: it asserts a wrong expectation so the
// analyzer can be verified against a real test failure.
func TestFormatDate(t *testing.T) {
	got := FormatDate(time.Now())
	want := "1970-01-01"
	if got != want {
		t.Errorf("FormatDate() = %s, want %s", got, want)
	}
}
