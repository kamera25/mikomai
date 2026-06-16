import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import jaTranslation from "./locales/ja/translation.json";
import enTranslation from "./locales/en/translation.json";

i18n
  .use(initReactI18next)
  .init({
    resources: {
      ja: { translation: jaTranslation },
      en: { translation: enTranslation },
    },
    lng: "ja", // Default language is Japanese
    fallbackLng: "ja",
    interpolation: {
      escapeValue: false, // React already protects from XSS
    },
  });

export default i18n;
