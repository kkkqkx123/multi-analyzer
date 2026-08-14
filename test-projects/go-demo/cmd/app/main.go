package main

import (
	"fmt"

	"github.com/google/uuid"

	"go-demo/internal/utils"
)

func main() {
	fmt.Println(utils.Greet("World"))
	fmt.Println(utils.Add(1, 2))
	fmt.Println("session:", uuid.NewString())
}
