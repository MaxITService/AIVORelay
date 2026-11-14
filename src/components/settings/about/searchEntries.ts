import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const aboutSearchEntries = [
  { id: "about", section: "about", labelKey: "sidebar.about", fallbackLabel: "About", keywords: ["about", "version", "update", "license", "credits", "о программе", "версия", "обновление", "лицензия"] },
  { id: "application-language", section: "about", anchor: "settings-application-language", labelKey: "appLanguage.title", fallbackLabel: "Application Language", keywords: ["interface language","ui language","locale","язык интерфейса","язык приложения"] },
  { id: "app-data-directory", section: "about", anchor: "settings-app-data-directory", labelKey: "settings.about.appDataDirectory.title", fallbackLabel: "App Data Directory", keywords: ["app data","data folder","models folder","storage location","папка данных","расположение данных"] },
] as const satisfies readonly SettingsSearchEntry[];
