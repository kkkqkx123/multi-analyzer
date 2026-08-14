// Deliberately contains ESLint and TypeScript issues for analyzer verification

export function add(a: number, b: number): number {
  const unusedSum = a + b; // error: 'unusedSum' is assigned a value but never used
  return a + b;
}

export function greet(name: string): string {
  return `Hello, ${name}`;
}

// type error: number is not assignable to string
export const version: string = 42;

// eslint error: no-explicit-any
export function processData(data: any): any {
  // eslint error: no-console
  console.log(data);
  return data;
}

let total = 0; // eslint error: prefer-const ('total' is never reassigned)
total = add(1, 2);
export { total };
