import { defineComponent, h, ref } from "vue";

export default defineComponent({
  props: ["name", "renderState"],
  setup(props) {
    const count = ref(0);
    if (props.renderState.input !== null) {
      props.renderState.output = { count: props.renderState.input.count + 1 };
    }
    const label = props.name ?? location.pathname;
    return () => h("main", [
      h("h1", label),
      h("button", { onClick: () => { count.value += 1; } }, String(count.value)),
    ]);
  },
});
