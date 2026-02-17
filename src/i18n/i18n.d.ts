// src/i18n/i18n.d.ts
import 'vue-i18n'
import type { I18nKey } from './types'

declare module 'vue-i18n' {
    export interface DefineLocaleMessage extends I18nKey { } // 如果是 vue-i18n v9
}

