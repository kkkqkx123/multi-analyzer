// Package utils provides small helpers used by the go-demo application.
package utils

import (
	"fmt"
	"time"
)

// Add returns the sum of a and b.
func Add(a, b int) int {
	return a + b
}

// Greet returns a greeting for the given name.
func Greet(name string) string {
	return fmt.Sprintf("Hello, %s!", name)
}

// FormatDate formats the given time as YYYY-MM-DD.
func FormatDate(t time.Time) string {
	return t.Format("2006-01-02")
}
