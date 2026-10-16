import type { ReactNode } from "react";
import { refractor } from "refractor/core";
// Only the languages EXTENSION_TO_LANGUAGE can pick: refractor/all registers
// ~300 grammars (600 kB) up front.
import typescript from "refractor/typescript";
import tsx from "refractor/tsx";
import javascript from "refractor/javascript";
import jsx from "refractor/jsx";
import json from "refractor/json";
import css from "refractor/css";
import scss from "refractor/scss";
import less from "refractor/less";
import markup from "refractor/markup";
import markdown from "refractor/markdown";
import python from "refractor/python";
import rust from "refractor/rust";
import go from "refractor/go";
import java from "refractor/java";
import kotlin from "refractor/kotlin";
import swift from "refractor/swift";
import ruby from "refractor/ruby";
import php from "refractor/php";
import c from "refractor/c";
import cpp from "refractor/cpp";
import csharp from "refractor/csharp";
import bash from "refractor/bash";
import yaml from "refractor/yaml";
import sql from "refractor/sql";
import ini from "refractor/ini";
import lua from "refractor/lua";
import r from "refractor/r";
import perl from "refractor/perl";
import makefile from "refractor/makefile";

for (const language of [
  typescript,
  tsx,
  javascript,
  jsx,
  json,
  css,
  scss,
  less,
  markup,
  markdown,
  python,
  rust,
  go,
  java,
  kotlin,
  swift,
  ruby,
  php,
  c,
  cpp,
  csharp,
  bash,
  yaml,
  sql,
  ini,
  lua,
  r,
  perl,
  makefile,
]) refractor.register(language);

const EXTENSION_TO_LANGUAGE: Record<string, string> = {
  ts: "typescript",
  tsx: "tsx",
  js: "javascript",
  jsx: "jsx",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  css: "css",
  scss: "scss",
  less: "less",
  html: "markup",
  htm: "markup",
  xml: "markup",
  vue: "markup",
  svelte: "markup",
  md: "markdown",
  markdown: "markdown",
  py: "python",
  rs: "rust",
  go: "go",
  java: "java",
  kt: "kotlin",
  swift: "swift",
  rb: "ruby",
  php: "php",
  c: "c",
  h: "c",
  cpp: "cpp",
  hpp: "cpp",
  cc: "cpp",
  cs: "csharp",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  yaml: "yaml",
  yml: "yaml",
  sql: "sql",
  toml: "ini",
  ini: "ini",
  lua: "lua",
  r: "r",
  pl: "perl",
};

/** Maps a file path to a refractor/Prism language name, or `null` when the
 * extension is unknown/unregistered — callers should fall back to plain text. */
export function guessLanguage(path: string): string | null {
  const name = path.split("/").pop() ?? path;
  if (/^makefile$/i.test(name)) return refractor.registered("makefile") ? "makefile" : null;
  const dot = name.lastIndexOf(".");
  if (dot === -1) return null;
  const lang = EXTENSION_TO_LANGUAGE[name.slice(dot + 1).toLowerCase()];
  return lang && refractor.registered(lang) ? lang : null;
}

type HastNode = {
  type: string;
  value?: string;
  properties?: { className?: string[] };
  children?: HastNode[];
};

function renderNodes(nodes: HastNode[] | undefined, keyPrefix: string): ReactNode {
  if (!nodes) return null;
  return nodes.map((node, i) => {
    const key = `${keyPrefix}-${i}`;
    if (node.type === "text") return node.value;
    return (
      <span key={key} className={node.properties?.className?.join(" ")}>
        {renderNodes(node.children, key)}
      </span>
    );
  });
}

/** Renders one line of code as syntax-highlighted spans (standard Prism
 * `.token.*` classes — colored per-theme in App.css), or plain text when the
 * language is unknown. */
export function highlightLine(content: string, language: string | null): ReactNode {
  if (!language) return content;
  try {
    const tree = refractor.highlight(content, language);
    return renderNodes(tree.children as HastNode[], "t");
  } catch {
    return content;
  }
}
