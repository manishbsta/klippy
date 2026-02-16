import { render } from "solid-js/web";
import { onCleanup, onMount } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import { App } from "./App";
import { useClipStore } from "./lib/store";
import "./styles/tailwind.css";

const Root = () => {
  const store = useClipStore();

  onMount(async () => {
    await store.init();
    const blockContextMenu = (event: MouseEvent) => {
      event.preventDefault();
    };
    document.addEventListener("contextmenu", blockContextMenu);

    const unlistenCreated = await listen("clips://created", async () => {
      await store.reload();
    });
    const unlistenUpdated = await listen("clips://updated", async () => {
      await store.reload();
    });
    const unlistenDeleted = await listen("clips://deleted", async () => {
      await store.reload();
    });

    onCleanup(() => {
      document.removeEventListener("contextmenu", blockContextMenu);
      unlistenCreated();
      unlistenUpdated();
      unlistenDeleted();
    });
  });

  return <App store={store} />;
};

render(() => <Root />, document.getElementById("root") as HTMLElement);
