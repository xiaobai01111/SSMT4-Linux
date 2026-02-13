import { createI18n } from 'vue-i18n'
import en from '../../internationalization/en.json'
import zhs from '../../internationalization/zhs.json'
import zht from '../../internationalization/zht.json'

export const messages = {
    en,
    zhs,
    zht,
}

export const i18n = createI18n({
    legacy: false,
    locale: 'en',
    fallbackLocale: 'en',
    messages,
})