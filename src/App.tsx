import { useEffect } from "react";
import { useApp } from "@/store/app";
import { WelcomeScreen } from "@/components/welcome/WelcomeScreen";
import { ConnectionForm } from "@/components/welcome/ConnectionForm";
import { Workspace } from "@/components/workspace/Workspace";
import { Toasts } from "@/components/common/Toasts";
import { useGlobalShortcuts } from "@/lib/shortcuts";

export default function App() {
  const screen = useApp((s) => s.screen);
  const init = useApp((s) => s.init);
  const form = useApp((s) => s.connectionForm);
  useEffect(() => { init(); }, [init]);
  useGlobalShortcuts();
  useEffect(() => {
    const block = (e: MouseEvent) => {
      // allow native menus only on inputs/editors; everything else uses our own menus
      const t = e.target as HTMLElement;
      if (t.closest("input, textarea, .cm-editor, .selectable")) return;
      e.preventDefault();
    };
    document.addEventListener("contextmenu", block);
    return () => document.removeEventListener("contextmenu", block);
  }, []);
  return (
    <>
      {screen === "welcome" ? <WelcomeScreen /> : <Workspace />}
      {form.open && <ConnectionForm />}
      <Toasts />
    </>
  );
}
