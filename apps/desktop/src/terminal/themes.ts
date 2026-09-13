// Terminal colour schemes: the Termoso pair plus the community set every
// terminal user knows (Solarized, Dracula, Gruvbox, Catppuccin, Nord, Tokyo
// Night, …) in xterm's 16-colour order. Settings store a scheme by `id`.

import type { ITheme } from "@xterm/xterm";
import { dark as appDark, emerald, light as appLight } from "@/theme/theme";

export interface TerminalTheme {
  id: string;
  name: string;
  /** Dark background — groups the gallery and picks the `auto` scheme. */
  dark: boolean;
  background: string;
  foreground: string;
  cursor: string;
  /** With alpha (#rrggbbaa). */
  selection: string;
  /** black, red, green, yellow, blue, magenta, cyan, white, then the bright eight. */
  ansi: readonly string[];
}

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1, 7), 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

const t = (
  id: string,
  name: string,
  background: string,
  foreground: string,
  cursor: string,
  selection: string,
  ansi: readonly string[],
): TerminalTheme => ({
  id,
  name,
  dark: luminance(background) < 0.5,
  background,
  foreground,
  cursor,
  selection,
  ansi,
});

export const TERMOSO_DARK = "termoso-dark";
export const TERMOSO_LIGHT = "termoso-light";
/** Follow the app colour scheme with the Termoso pair. */
export const AUTO_THEME = "auto";

// prettier-ignore
const termosoDark = t(TERMOSO_DARK, "Termoso Dark", appDark.lowest, appDark.text, emerald.main, "#2BB88452", [
  "#1A1E2B", "#F25E61", "#2BB884", "#F2C94C", "#5AA9E6", "#C58AF9", "#4FD1C5", "#C9CDDA",
  "#5A6076", "#FF8285", "#5FD0A4", "#FFDB7A", "#8CC5F0", "#DBB3FF", "#84E7DD", "#F2F4FA",
]);
// prettier-ignore
const termosoLight = t(TERMOSO_LIGHT, "Termoso Light", appLight.high, appLight.text, emerald.dark, "#1F8F6647", [
  "#171B26", "#C93B3E", "#1F8F66", "#B08A12", "#2C6B9C", "#8A4FCF", "#1C8C82", "#9AA3B2",
  "#5F6B7C", "#F25E61", "#2BB884", "#D6A81C", "#3D86BF", "#A66BE8", "#28A89C", "#CBD2DC",
]);

// prettier-ignore
export const terminalThemes: readonly TerminalTheme[] = [
  termosoDark,
  termosoLight,
  t("flexoki-dark", "Flexoki Dark", "#100f0f", "#cecdc3", "#cecdc3", "#cecdc380", [
    "#100f0f", "#af3029", "#66800b", "#ad8301", "#205ea6", "#a02f6f", "#24837b", "#878580",
    "#6f6e69", "#d14d41", "#879a39", "#d0a215", "#4385be", "#ce5d97", "#3aa99f", "#cecdc3",
  ]),
  t("flexoki-light", "Flexoki Light", "#fffcf0", "#100f0f", "#100f0f", "#100f0f80", [
    "#100f0f", "#d14d41", "#879a39", "#d0a215", "#4385be", "#ce5d97", "#3aa99f", "#fffcf0",
    "#6f6e69", "#af3029", "#66800b", "#ad8301", "#205ea6", "#a02f6f", "#24837b", "#f2f0e5",
  ]),
  t("kanagawa-wave", "Kanagawa Wave", "#1f1f28", "#dcd7ba", "#c8c093", "#d7ba8080", [
    "#16161d", "#c34043", "#5d864d", "#c0a36e", "#7e9cd8", "#6f549b", "#6a9589", "#c8c093",
    "#727169", "#e9585b", "#98bb6c", "#e6c384", "#7fb4ca", "#a18bc4", "#8bc5b9", "#dcd7ba",
  ]),
  t("kanagawa-dragon", "Kanagawa Dragon", "#181616", "#c5c9c5", "#c8c093", "#c9c58080", [
    "#0d0c0c", "#c4746e", "#596f44", "#c4b28a", "#8ba4b0", "#9c5f9f", "#5a948f", "#e6e2c7",
    "#a6a69c", "#e88a83", "#9ccd9c", "#e6c384", "#7fb4ca", "#d7bdd9", "#77c7c0", "#fffad9",
  ]),
  t("kanagawa-lotus", "Kanagawa Lotus", "#f2ecbc", "#545464", "#43436c", "#54648080", [
    "#1f1f28", "#c84053", "#6f894e", "#cbbc41", "#4871af", "#e56792", "#5fa99c", "#545464",
    "#555448", "#e65569", "#88a760", "#e5d978", "#6693bf", "#c299ff", "#7cc4b0", "#43436c",
  ]),
  t("hacker-blue", "Hacker Blue", "#010515", "#11b7ff", "#10b6ff", "#c1e4ff80", [
    "#1e2644", "#0d7eaf", "#529cbd", "#00608a", "#034765", "#648593", "#7ad6ff", "#d1f1ff",
    "#2f385a", "#0fbdff", "#6fb7d3", "#43c3f4", "#5cc0e6", "#7ab2c7", "#4dceff", "#fefefe",
  ]),
  t("hacker-green", "Hacker Green", "#020f01", "#16b10e", "#15d00d", "#d4ffc180", [
    "#243128", "#14af0d", "#86ce83", "#068a00", "#50be4b", "#4b7649", "#a8f0a4", "#d6fcd4",
    "#36463b", "#28f11d", "#90e08b", "#4cf443", "#63e65c", "#a8d9a5", "#59ea51", "#fefefe",
  ]),
  t("hacker-red", "Hacker Red", "#200000", "#b10e0e", "#b00d0d", "#ebc1ff80", [
    "#381313", "#b00d0d", "#5d2020", "#890000", "#be4a4a", "#7a3b3b", "#ff7979", "#ffd2d2",
    "#993f3f", "#ff1111", "#d26d6d", "#f54242", "#e55b5b", "#c87b7b", "#ff4d4d", "#fefefe",
  ]),
  t("everforest-dark", "Everforest Dark", "#282e32", "#d3c6aa", "#d3c6aa", "#d3c6aa80", [
    "#42494e", "#a1484a", "#778e54", "#ba9e68", "#388084", "#906378", "#6ca37a", "#c0dac6",
    "#575656", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#e8f4eb",
  ]),
  t("everforest-light", "Everforest Light", "#fefbf1", "#5c6a72", "#5c6a72", "#5c6a7280", [
    "#42494e", "#d2413e", "#919d45", "#d89902", "#2b7ba7", "#bc72a5", "#50b08c", "#c8d0c9",
    "#575656", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d7e2d8",
  ]),
  t("night-owl", "Night Owl", "#011627", "#d6deeb", "#80a4c2", "#d6deeb80", [
    "#072945", "#ef5350", "#22da6e", "#c5e478", "#82aaff", "#c792ea", "#21c7a8", "#e1f1ff",
    "#575656", "#ff7472", "#40fa8d", "#ffeb95", "#a0beff", "#daa4ff", "#7fdbca", "#ffffff",
  ]),
  t("light-owl", "Light Owl", "#fbfbfb", "#403f53", "#90a7b2", "#403f5380", [
    "#403f53", "#de3d3b", "#08916a", "#e0af02", "#288ed7", "#d6438a", "#2aa298", "#e8e5e5",
    "#57566d", "#fa5d5b", "#1abf90", "#f4c315", "#3ca3ec", "#f559a4", "#39c6ba", "#f6f6f6",
  ]),
  t("aura", "Aura", "#21202e", "#edecee", "#edecee", "#edecee80", [
    "#1c1b22", "#ff6767", "#4deeb8", "#f4be77", "#5b72ee", "#a277ff", "#51fafa", "#dddbfa",
    "#4d4d4d", "#ffa285", "#99ffdd", "#ffd49d", "#8296ff", "#b592ff", "#8cffff", "#ffffff",
  ]),
  t("rose-pine", "Rosé Pine", "#191724", "#e0def4", "#e0def4", "#e0def480", [
    "#26233a", "#be5d78", "#78b1bb", "#e4ad5f", "#266782", "#9c7ac5", "#c28987", "#e0def4",
    "#6e6a86", "#eb6f92", "#9ccfd8", "#f6c177", "#4386a1", "#d5b2ff", "#ebbcba", "#f0eeff",
  ]),
  t("rose-pine-moon", "Rosé Pine Moon", "#232136", "#e0def4", "#e0def4", "#e0def480", [
    "#393552", "#be5d78", "#78b1bb", "#e4ad5f", "#307997", "#9c7ac5", "#da8c8a", "#e0def4",
    "#6e6a86", "#eb6f92", "#9ccfd8", "#f6c177", "#3e8fb0", "#d5b2ff", "#ebbcba", "#f0eeff",
  ]),
  t("rose-pine-dawn", "Rosé Pine Dawn", "#faf4ed", "#575279", "#575279", "#57527980", [
    "#c9c0b9", "#b4637a", "#56949f", "#ea9d34", "#286983", "#875fb5", "#c47874", "#3e3a5b",
    "#e5dcd3", "#e8829f", "#a0d5df", "#f6c177", "#46afd9", "#be96ec", "#dd8f8c", "#aaa2e2",
  ]),
  t("cobalt2", "Cobalt2", "#132738", "#ffffff", "#f0cc09", "#ffffff80", [
    "#000000", "#ff0000", "#38de21", "#ffe50a", "#1460d2", "#ff4387", "#00bbbb", "#cfcfcf",
    "#555555", "#ff757a", "#69fb79", "#fff285", "#77adff", "#ff92cc", "#6bffff", "#ffffff",
  ]),
  t("octocat-dark", "Octocat Dark", "#101216", "#8b949e", "#c9d1d9", "#8b949e80", [
    "#000000", "#f78166", "#56d364", "#e3b341", "#6ca4f8", "#db61a2", "#2b7489", "#DADADA",
    "#4d4d4d", "#ffb5a5", "#69fb79", "#ffcf5f", "#b0d0ff", "#ff92cc", "#54d8ff", "#ffffff",
  ]),
  t("octocat-light", "Octocat Light", "#f4f4f4", "#3e3e3e", "#3f3f3f", "#3e3e3e80", [
    "#000000", "#970b16", "#07962a", "#f1d007", "#0053b9", "#e94691", "#89d1ec", "#dfdddd",
    "#666666", "#de0000", "#87d5a2", "#ffe689", "#2e6cba", "#ffa29f", "#1cfafe", "#ffffff",
  ]),
  t("ayu-dark", "Ayu Dark", "#0f1419", "#e6e1cf", "#f29718", "#e6e1cf80", [
    "#000000", "#ff3333", "#b8cc52", "#dbb012", "#36a3d9", "#df7a80", "#6ceedf", "#ababab",
    "#323232", "#ff8181", "#eafe84", "#ffe174", "#68d5ff", "#ffa3aa", "#94fff1", "#ffffff",
  ]),
  t("ayu-light", "Ayu Light", "#fafafa", "#5c6773", "#ff6a00", "#5c677380", [
    "#000000", "#ff3333", "#319900", "#f29718", "#41a6d9", "#e07ead", "#1dd1b0", "#dfdddd",
    "#323232", "#ff5959", "#b8e532", "#ffc94a", "#73d8ff", "#ffa3aa", "#7ff1cb", "#ffffff",
  ]),
  t("cyberpunk", "Cyberpunk", "#332a57", "#e5e5e5", "#21f6bc", "#e5e5e580", [
    "#100c1d", "#ff7092", "#01da7f", "#ddd84f", "#00a0d6", "#be4eee", "#28e3cc", "#eaeaea",
    "#221b3a", "#ff9999", "#00f890", "#fff787", "#28bfe9", "#e4a7ff", "#95fff2", "#ffffff",
  ]),
  t("cyberpunk-scarlet", "Cyberpunk Scarlet", "#101116", "#ff0055", "#00ffc8", "#ff005580", [
    "#272831", "#c5123d", "#019540", "#ffed30", "#00b0ff", "#c77bff", "#4ae3c2", "#6e7281",
    "#3f4352", "#ff8aa4", "#44d884", "#fff7ac", "#c2ecff", "#e6aefe", "#7bffe2", "#ffffff",
  ]),
  t("romania-night", "Romania Night", "#0b141f", "#f5f5ea", "#f7d382", "#f5f5ea80", [
    "#0b141f", "#b50011", "#124150", "#8d562d", "#444f60", "#5b2928", "#80252d", "#917d7c",
    "#1d2d41", "#de1016", "#b16749", "#d74d29", "#e69148", "#c2b1ad", "#f7d382", "#f5f5ea",
  ]),
  t("romania-day", "Romania Day", "#f5e8e7", "#632228", "#d7b25e", "#63222880", [
    "#300f0d", "#b50011", "#1c817b", "#b4a667", "#798cb1", "#cf7751", "#fd7d89", "#bfb3a4",
    "#5b2c29", "#000000", "#5bc2b1", "#ffcd87", "#e69148", "#c2b1ad", "#ddb763", "#fffefd",
  ]),
  t("aubergine", "Aubergine", "#2c001e", "#eeeeec", "#bbbbbb", "#eeeeec80", [
    "#47233b", "#ae3120", "#4e9a06", "#c4a000", "#3465a4", "#8f6696", "#5dbdac", "#cdc2b6",
    "#682e59", "#e95420", "#8ae234", "#fce94f", "#729fcf", "#b188ad", "#34e2e2", "#eeeeec",
  ]),
  t("peach-fresh", "Peach Fresh", "#f5c19e", "#520701", "#b33a00", "#e6760180", [
    "#220200", "#cb4434", "#65a06a", "#e6a200", "#3c5672", "#7a1a48", "#517e9a", "#fddac3",
    "#61201b", "#db6355", "#7db882", "#ffcf5c", "#5f7c9d", "#d06397", "#82adc9", "#ffffff",
  ]),
  t("1984-dark", "1984 Dark", "#0d1030", "#dedee1", "#dedee1", "#dedee180", [
    "#2a2a54", "#db229d", "#89b15c", "#ddca34", "#377fd2", "#c124db", "#4497a2", "#dedef4",
    "#363666", "#fd27b5", "#9ed262", "#f3e049", "#44a6e0", "#e42cf4", "#6bcdda", "#ececf9",
  ]),
  t("1984-light", "1984 Light", "#e4ecf4", "#353247", "#353247", "#35324780", [
    "#1f2022", "#f93bb8", "#2cb569", "#ea9033", "#0380d9", "#cf29ea", "#0698a4", "#dedef4",
    "#1f2022", "#ff89d6", "#59d08e", "#ffb76d", "#6dc2ff", "#e26eff", "#3bcfdb", "#fbfbfe",
  ]),
  t("winter-night", "Winter Night", "#00192c", "#e4d5cc", "#f8cfa6", "#e4d5cc80", [
    "#1e2c37", "#933911", "#51876b", "#cc9867", "#3e5f7e", "#82658a", "#7cb298", "#f9edcc",
    "#35424c", "#d85751", "#7b9a8a", "#e7cf91", "#6098aa", "#b690e6", "#94d2b5", "#fff9ea",
  ]),
  t("winter-day", "Winter Day", "#ece3d1", "#1a2720", "#5c6370", "#abb2bf80", [
    "#685b3f", "#9c3b12", "#5a9f79", "#dca067", "#546f88", "#5d4864", "#2e8e9a", "#d1c6af",
    "#857554", "#b26848", "#7cb596", "#ffc48d", "#6d8fb0", "#d3b3dd", "#56b6c2", "#fff7e4",
  ]),
  t("tokyo-night", "Tokyo Night", "#1a1b26", "#cdd6ff", "#c0caf5", "#c0caf580", [
    "#373947", "#f7768e", "#92ba68", "#e8be80", "#5d91ff", "#a079e5", "#7dcfff", "#6d759a",
    "#414868", "#ff8da2", "#b8e785", "#ffd292", "#739fff", "#b288ff", "#b1e2ff", "#dbe2ff",
  ]),
  t("tokyo-day", "Tokyo Day", "#e1e2e7", "#2f54a8", "#3760bf", "#3760bf80", [
    "#f4f4fb", "#f52a65", "#506d31", "#a07433", "#4782d4", "#9854f1", "#004b65", "#465484",
    "#bfc2d0", "#ff6692", "#819966", "#9f8662", "#5898f0", "#cba4ff", "#00a6de", "#b6c4f9",
  ]),
  t("catppuccin-latte", "Catppuccin Latte", "#eff1f5", "#4c4f69", "#dc8a78", "#dc8a7880", [
    "#5c5f77", "#c41037", "#389524", "#d58515", "#3167d2", "#d26eb7", "#09908e", "#acb0be",
    "#7a7d92", "#e6224c", "#4cb435", "#eb9926", "#2f75ff", "#ff99e3", "#1eabb3", "#d3d7e3",
  ]),
  t("catppuccin-mocha", "Catppuccin Mocha", "#1e1e2e", "#cdd6f4", "#f5e0dc", "#f5e0dc80", [
    "#45475a", "#ad626b", "#95cb91", "#b09e76", "#4c82d9", "#bc81ae", "#71b2aa", "#5a5d74",
    "#64698a", "#fb99b4", "#aeeca9", "#ffe7b4", "#67bfff", "#ffccf1", "#a2f3e5", "#d7ddf3",
  ]),
  t("diwali", "Diwali", "#311652", "#fdf6f6", "#ffa900", "#ffa90080", [
    "#1e0b34", "#ad0656", "#00b45d", "#f8d203", "#4548e1", "#e869ea", "#00d5dd", "#856da2",
    "#5a3f7e", "#f02889", "#35ef96", "#ffe769", "#7578ff", "#fc99ff", "#47eef4", "#f8efff",
  ]),
  t("movember", "Movember", "#1f0900", "#ffe7c3", "#e6c03c", "#3b262680", [
    "#421400", "#a43219", "#4b6d21", "#caa630", "#1e4888", "#7c2f4a", "#357b6e", "#cdba9e",
    "#561900", "#d04325", "#749c43", "#fac04f", "#4080e0", "#b44c72", "#52bca8", "#fff1dd",
  ]),
  t("atom-one-dark", "Atom One Dark", "#1e2127", "#abb2bf", "#5c6370", "#abb2bf80", [
    "#000000", "#ca6169", "#82a568", "#bf8c5d", "#56a2e1", "#b76ccd", "#4e9aa3", "#c5cbd6",
    "#5c6370", "#e77c84", "#b4e294", "#e9b17b", "#7ec5ff", "#db8df2", "#64cfdd", "#ffffff",
  ]),
  t("atom-one-light", "Atom One Light", "#f9f9f9", "#383a42", "#383a42", "#383a4280", [
    "#000000", "#e45649", "#4c9b4b", "#c99525", "#4078f2", "#a626a4", "#0184bc", "#b8b9bf",
    "#474747", "#ff7468", "#74ca72", "#dba633", "#6a99ff", "#c142bf", "#00b1fd", "#ffffff",
  ]),
  t("halloween", "Halloween", "#22012b", "#e5e5e5", "#ffa900", "#ffa90080", [
    "#100015", "#a31736", "#887225", "#ff7600", "#5c50a6", "#6d008d", "#ae4fa4", "#c7c7c6",
    "#694a71", "#ff2e5d", "#c3a640", "#ffa85e", "#8e7cff", "#c500ff", "#f491ea", "#fafafa",
  ]),
  t("dia-de-muertos", "Dia De Muertos", "#fffdf5", "#281f63", "#b33a00", "#e6760180", [
    "#020018", "#eb1670", "#23816c", "#fb6633", "#5c439e", "#9d2686", "#e29b00", "#d2cbb4",
    "#3f3b64", "#f63b7b", "#28aa8b", "#ffa85e", "#7a54df", "#ff81c7", "#ffbe30", "#f1ebd9",
  ]),
  t("gruvbox-dark", "Gruvbox Dark", "#282828", "#ebdbb2", "#ebdbb2", "#ebdbb280", [
    "#151515", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a", "#c3b198",
    "#695c50", "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#f59db5", "#8ec07c", "#ebdbb2",
  ]),
  t("gruvbox-light", "Gruvbox Light", "#fbf1c7", "#282828", "#282828", "#28282880", [
    "#dfd6b1", "#9d0006", "#79740e", "#b57614", "#076678", "#8f3f71", "#427b58", "#3c3836",
    "#9d8374", "#cc241d", "#98971a", "#d79921", "#458588", "#d180a5", "#689d69", "#7c6f64",
  ]),
  t("material-dark", "Material Dark", "#232322", "#e5e5e5", "#16afca", "#e5e5e580", [
    "#040404", "#b7141f", "#457b24", "#f6981e", "#134eb2", "#560088", "#0e717c", "#efefef",
    "#424242", "#e83b3f", "#7aba3a", "#ffea2e", "#54a4f3", "#aa4dbc", "#26bbd1", "#d9d9d9",
  ]),
  t("material-light", "Material Light", "#eaeaea", "#2f2f2f", "#16afca", "#23232280", [
    "#000000", "#b7141f", "#457b24", "#f6981e", "#134eb2", "#560088", "#0e717c", "#f5f5f5",
    "#424242", "#e83b3f", "#7aba3a", "#ffea2e", "#54a4f3", "#aa4dbc", "#26bbd1", "#d9d9d9",
  ]),
  t("manhattan", "Manhattan", "#0a0a0a", "#b8b9b4", "#dfe0db", "#b8b9b480", [
    "#0a0a0a", "#840f02", "#5e5e5c", "#c3a421", "#727370", "#d1d1cb", "#4f4f4c", "#aaaba6",
    "#2e2e2c", "#d15510", "#636361", "#d8b741", "#777875", "#d6d6d0", "#696966", "#afb0ab",
  ]),
  t("plastic-world", "Plastic World", "#ea56c1", "#ffe9da", "#bafc8b", "#ffe9da80", [
    "#581743", "#de0000", "#99d76d", "#f2d66f", "#eb8dd7", "#9b37ff", "#68b9dc", "#f6b8ec",
    "#86316b", "#ff7ba3", "#d3ffaf", "#ffe383", "#fe9be8", "#a65cf0", "#b0e6fe", "#ffe6f7",
  ]),
  t("basic", "Basic", "#ffffff", "#000000", "#7f7f7f", "#00000080", [
    "#2e2e2e", "#c61a1a", "#007900", "#999900", "#0f48cd", "#b200b2", "#3fc1dd", "#acacac",
    "#757575", "#ff3e3e", "#00b300", "#d4d400", "#316fff", "#ff60c9", "#6ce5ff", "#cac5c5",
  ]),
  t("homebrew", "Homebrew", "#000000", "#00ff00", "#23ff18", "#00ff0080", [
    "#2e2e2e", "#c93434", "#348e48", "#e09e00", "#0031e0", "#e235ff", "#3fc1dd", "#d0cfcf",
    "#5b5b5b", "#ff6767", "#31ff31", "#ffdca8", "#4465da", "#ff5fc8", "#8debff", "#e6e6e6",
  ]),
  t("grass", "Grass", "#13773d", "#fff0a5", "#8c2800", "#fff0a580", [
    "#000000", "#9a183c", "#6eb95e", "#ffa673", "#00378a", "#771361", "#3bcbea", "#939393",
    "#393939", "#e0692f", "#b2ffa2", "#ffc27b", "#2380c4", "#ec88c2", "#70e4ff", "#ffffff",
  ]),
  t("man-page", "Man Page", "#fef49c", "#000000", "#7f7f7f", "#00000080", [
    "#383838", "#9a183c", "#009100", "#be6600", "#114695", "#b72fb9", "#3bcbea", "#959595",
    "#a7a7a7", "#e0692f", "#00b400", "#ffb571", "#3392d6", "#ec88c2", "#70e4ff", "#dadada",
  ]),
  t("novel", "Novel", "#dfdbc3", "#3b2322", "#73635a", "#3b232280", [
    "#000000", "#d30f0f", "#00933b", "#d38b40", "#00528e", "#cc32cf", "#26c3e6", "#a6a6a6",
    "#5c5c5c", "#e0692f", "#00b400", "#fff284", "#3ba6f3", "#ec88c2", "#38daff", "#f2f2f2",
  ]),
  t("ocean", "Ocean", "#224fbc", "#ffffff", "#7f7f7f", "#ffffff80", [
    "#000000", "#881616", "#399518", "#dda114", "#00a3ff", "#a83aff", "#28ccd6", "#d3d3d3",
    "#d2d2d2", "#ff7658", "#00ff47", "#f5c147", "#79ceff", "#ea6fff", "#58f4ff", "#ffffff",
  ]),
  t("pro", "Pro", "#000000", "#f2f2f2", "#4d4d4d", "#f2f2f280", [
    "#2e2e2e", "#c93434", "#348e48", "#e09e00", "#002bc7", "#e235ff", "#3fc1dd", "#d0cfcf",
    "#5b5b5b", "#ff6767", "#31ff31", "#ffdca8", "#4465da", "#ff5fc8", "#8debff", "#e6e6e6",
  ]),
  t("red-sands", "Red Sands", "#7a251e", "#d7c9a7", "#ffffff", "#d7c9a780", [
    "#000000", "#d30e0e", "#58aa47", "#ffa673", "#0072ff", "#ff57ee", "#3bcbea", "#e6e6e6",
    "#606060", "#e0692f", "#b2ffa2", "#ffc27b", "#0193fc", "#ffbce2", "#70e4ff", "#ffffff",
  ]),
  t("solarized-dark", "Solarized Dark", "#002b36", "#839496", "#657779", "#65777980", [
    "#11586a", "#dc322f", "#8ea20a", "#b58900", "#268bd2", "#c41f6f", "#2aa198", "#e7e0cc",
    "#003b4a", "#f15c59", "#677558", "#7e7a61", "#83a8ad", "#886cc4", "#72b6b6", "#fdf6e3",
  ]),
  t("solarized-light", "Solarized Light", "#fdf6e3", "#657b83", "#657b83", "#657b8380", [
    "#073642", "#dc322f", "#8fa30a", "#b58900", "#268bd2", "#c41f6f", "#2aa198", "#e6e0cb",
    "#00252f", "#e05319", "#667558", "#7e7960", "#83a8ad", "#886cc4", "#72b6b6", "#fff0c7",
  ]),
  t("silver-aerogel", "Silver Aerogel", "#919191", "#000000", "#d9d9d9", "#00000080", [
    "#262424", "#ca3535", "#09672f", "#ffbe73", "#004582", "#cf41e6", "#45adce", "#eae9e9",
    "#b6b6b6", "#ff7658", "#57cc5c", "#fff06b", "#487bff", "#ff60c9", "#2cc6f7", "#ffffff",
  ]),
  t("dracula", "Dracula", "#282a36", "#f8f8f2", "#bbbbbb", "#f8f8f280", [
    "#000000", "#e04242", "#45e16c", "#e3ec7d", "#9b7dc6", "#e469b0", "#8be9fd", "#cac5c5",
    "#4a4a4a", "#ff5555", "#b5ffc8", "#fff9c8", "#c8a1ff", "#ff8cce", "#c8f5ff", "#ffffff",
  ]),
  t("monokai", "Monokai", "#0c0c0c", "#d9d9d9", "#fc971f", "#d9d9d980", [
    "#1a1a1a", "#dd0056", "#92d526", "#fd971f", "#874deb", "#ea095a", "#48bfd8", "#c4c5b5",
    "#625e4c", "#ff3382", "#a6f12f", "#e0d561", "#9d65ff", "#ff116d", "#58d1eb", "#f6f6ef",
  ]),
  t("nord-light", "Nord Light", "#e5e9f0", "#414858", "#88c0d0", "#41485880", [
    "#2c3344", "#ae545d", "#8ca377", "#dabe84", "#718fae", "#95728e", "#78acbb", "#d8dee9",
    "#4c556a", "#d97982", "#a3be8b", "#eacb8a", "#a4c7e9", "#b48dac", "#8fbcbb", "#eceff4",
  ]),
  t("nord-dark", "Nord Dark", "#2e3440", "#d8dee9", "#eceff4", "#eceff480", [
    "#3b4252", "#ae545d", "#8ca377", "#dabe84", "#718fae", "#95728e", "#78acbb", "#d8dee9",
    "#4c556a", "#d97982", "#a3be8b", "#eacb8a", "#a4c7e9", "#b48dac", "#8fbcbb", "#eceff4",
  ]),
];

const byId = new Map(terminalThemes.map((th) => [th.id, th]));

export function terminalThemeById(id: string): TerminalTheme | undefined {
  return byId.get(id);
}

/** Scheme to render for a settings value; unknown ids fall back to the Termoso pair. */
export function resolveTerminalTheme(
  id: string | undefined,
  scheme: "dark" | "light",
): TerminalTheme {
  const fallback = scheme === "light" ? termosoLight : termosoDark;
  if (!id || id === AUTO_THEME) return fallback;
  return byId.get(id) ?? fallback;
}

export function toXtermTheme(th: TerminalTheme): ITheme {
  const [black, red, green, yellow, blue, magenta, cyan, white, ...bright] = th.ansi;
  return {
    background: th.background,
    foreground: th.foreground,
    cursor: th.cursor,
    cursorAccent: th.background,
    selectionBackground: th.selection,
    selectionInactiveBackground: th.selection.slice(0, 7) + "33",
    black,
    red,
    green,
    yellow,
    blue,
    magenta,
    cyan,
    white,
    brightBlack: bright[0],
    brightRed: bright[1],
    brightGreen: bright[2],
    brightYellow: bright[3],
    brightBlue: bright[4],
    brightMagenta: bright[5],
    brightCyan: bright[6],
    brightWhite: bright[7],
  };
}
