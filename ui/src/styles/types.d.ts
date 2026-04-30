// Type declarations for CSS module imports. Vite's default behaviour exposes
// each *.module.css as a typed record of class names.

declare module '*.module.css' {
  const classes: { readonly [key: string]: string };
  export default classes;
}
