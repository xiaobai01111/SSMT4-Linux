import { createI18n } from 'vue-i18n'
import { en, zhs, zht } from '../../internationalization'

export const messages = {
    en,
    zhs,
    zht,
}

const missingWithFallbackLogged = new Set<string>()
const missingWithoutFallbackLogged = new Set<string>()

const getByPath = (obj: Record<string, unknown>, key: string): string | undefined => {
    let current: unknown = obj
    for (const segment of key.split('.')) {
        if (!current || typeof current !== 'object' || !(segment in current)) {
            return undefined
        }
        current = (current as Record<string, unknown>)[segment]
    }
    return typeof current === 'string' ? current : undefined
}

export const i18n = createI18n({
    legacy: false,
    locale: 'zhs',
    fallbackLocale: ['zhs', 'en', 'zht'],
    messages,
    missing: (locale, key) => {
        const dedupeKey = `${locale}:${key}`
        const fallback = getByPath(zhs as Record<string, unknown>, key)
        if (typeof fallback === 'string') {
            if (import.meta.env.DEV && !missingWithFallbackLogged.has(dedupeKey)) {
                missingWithFallbackLogged.add(dedupeKey)
                console.warn(`[i18n] Missing key '${key}' in locale '${locale}', fallback to zhs.`)
            }
            return fallback
        }
        if (import.meta.env.DEV && !missingWithoutFallbackLogged.has(dedupeKey)) {
            missingWithoutFallbackLogged.add(dedupeKey)
            console.warn(`[i18n] Missing key '${key}' in locale '${locale}', and zhs fallback is also missing.`)
        }
        return key
    },
})
