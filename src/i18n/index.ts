import { createI18n } from 'vue-i18n'
import en from '../../internationalization/en.json'
import zhs from '../../internationalization/zhs.json'
import zht from '../../internationalization/zht.json'

export const messages = {
    en,
    zhs,
    zht,
}

const getByPath = (obj: Record<string, any>, key: string): string | undefined => {
    return key.split('.').reduce<any>((acc, segment) => {
        if (acc && typeof acc === 'object' && segment in acc) {
            return acc[segment]
        }
        return undefined
    }, obj)
}

export const i18n = createI18n({
    legacy: false,
    locale: 'en',
    fallbackLocale: ['zhs', 'en'],
    messages,
    missing: (locale, key) => {
        const fallback = getByPath(zhs as Record<string, any>, key)
        if (typeof fallback === 'string') {
            console.warn(`[i18n] Missing key '${key}' in locale '${locale}', fallback to zhs.`)
            return fallback
        }
        console.warn(`[i18n] Missing key '${key}' in locale '${locale}', and zhs fallback is also missing.`)
        return key
    },
})
