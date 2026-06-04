/**
 * 应用版本号 — 单一来源
 * 编译时从 package.json 注入，build 时 Vite 会内联具体版本号到产物
 *
 * 用法：
 *   import { APP_VERSION } from "../utils/version";
 *   <span>v{APP_VERSION}</span>
 *
 * 为什么单独抽出来：避免在多个组件里硬编码版本号字符串，
 * 升级时只改 package.json 一处即可全局生效。
 */
import packageJson from "../../package.json";

export const APP_VERSION: string = packageJson.version;
