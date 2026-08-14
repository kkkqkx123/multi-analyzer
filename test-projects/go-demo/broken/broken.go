//go:build broken

// Package broken contains deliberate compile errors. It is only compiled when
// the "broken" build tag is enabled: go build -tags broken ./...
package broken

import "fmt"

// UndeclaredUsage references an undefined identifier.
func UndeclaredUsage() {
	fmt.Println(unknownVariable) // undefined: unknownVariable
}

// typeMismatch returns a string where an int is expected.
func typeMismatch() int {
	return "not an int" // cannot use "not an int" as int value
}
