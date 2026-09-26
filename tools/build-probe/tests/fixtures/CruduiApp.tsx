import React from "react";
import { compileForm, createForm } from "@crudui/generator-core";
import { Form } from "@crudui/generator-react";

const spec = {
  type: "group",
  properties: {
    rows: {
      type: "group",
      multiple: true,
      properties: {
        title: { type: "text" },
        children: {
          type: "group",
          multiple: true,
          properties: { title: { type: "text" } },
        },
      },
    },
  },
};

export default function App() {
  const form = createForm(compileForm(spec), {}, {
    language: "en",
    keyPrefix: "public-ssr",
    idPrefix: "public-ssr",
  });
  return <Form form={form} />;
}
