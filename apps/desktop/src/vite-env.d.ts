/// <reference types="vite/client" />

// CSS Modules type shim
declare module '*.module.css' {
  const classes: { readonly [key: string]: string };
  export default classes;
}

// Image assets
declare module '*.svg' {
  const src: string;
  export default src;
}
declare module '*.png' {
  const src: string;
  export default src;
}
