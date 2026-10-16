import { createElement, lazy, Suspense, type ComponentProps, type ComponentType } from "react";

/** A named export loaded on first render, each with its own Suspense boundary
 * (so a first load never hides the rest of the screen):
 * `lazyNamed(() => import("./X"), "X")`. Renders nothing while loading. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function lazyNamed<K extends string, M extends Record<K, ComponentType<any>>>(
  loader: () => Promise<M>,
  name: K,
) {
  const Lazy = lazy<M[K]>(async () => ({ default: (await loader())[name] }));
  return function LazyView(props: ComponentProps<M[K]>) {
    return createElement(Suspense, { fallback: null }, createElement(Lazy as ComponentType<ComponentProps<M[K]>>, props));
  };
}
