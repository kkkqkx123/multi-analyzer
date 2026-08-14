// Deliberately contains ESLint and TypeScript issues for analyzer verification

import { add, greet } from "@pnpm-turbo/utils";

// type error: number is not assignable to string
const greeting: string = add(1, 2);

const unusedVar = "hello"; // error: 'unusedVar' is assigned a value but never used

// eslint error: no-console
console.log(greeting);

interface User {
  name: string;
  age: number;
}

// type error: property 'age' is missing in type '{ name: string; }'
const user: User = { name: "Alice" };

// eslint error: no-explicit-any
function render(id: any): string {
  return greet(String(id)) + user.name;
}

// eslint error: prefer-const
let message = "welcome";
message = render("x");

console.log(message);
