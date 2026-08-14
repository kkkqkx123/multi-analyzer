// Package greeter contains an intentional fmt.Printf verb mismatch so that
// `go vet` reports a warning the analyzer can pick up.
package greeter

import "fmt"

// Greet prints the Go version using the wrong format verb on purpose.
func Greet() {
	fmt.Printf("%s\n", 42) // vet: %s format has arg 42 of wrong type int
}
