<script setup lang="ts">
import { appSettings } from '../store'
import { openFileDialog } from '../api';
import { useI18n } from 'vue-i18n';

const { t } = useI18n()

const selectCacheDir = async () => {
  const selected = await openFileDialog({
    directory: true,
    multiple: false,
    title: '选择 SSMT 缓存文件夹'
  });

  if (selected && typeof selected === 'string') {
    appSettings.cacheDir = selected;
  }
};
</script>

<template>
  <div class="page-container" style="padding: 24px 24px 56px 24px;">

    <el-card>
      <template #header>
        <div class="card-header">
          <span>基础设置</span>
        </div>
      </template>
      <el-form label-width="140px">
        <el-form-item :label="t('settings.language')">
          <el-select v-model="appSettings.locale" placeholder="Select language" style="width: 200px">
            <el-option label="简体中文" value="zhs" />
            <el-option label="繁體中文" value="zht" />
            <el-option label="English" value="en" />
          </el-select>
        </el-form-item>
        <el-form-item label="SSMT缓存文件夹">
          <div style="display: flex; gap: 10px; width: 100%;">
            <el-input v-model="appSettings.cacheDir" placeholder="请选择或输入缓存文件夹路径" />
            <el-button @click="selectCacheDir">选择文件夹</el-button>
          </div>
        </el-form-item>
        <el-form-item label="GitHub Token">
          <el-input v-model="appSettings.githubToken" placeholder="可选: 填写Token可提高API请求限额" type="password"
            show-password />
        </el-form-item>
      </el-form>
    </el-card>

    <br />

    <el-card>
      <template #header>
        <div class="card-header">
          <span>外观设置</span>
        </div>
      </template>

      <el-form label-width="140px">
        <div class="settings-divider">内容区样式 (Content)</div>
        <el-form-item label="不透明度 (Opacity)">
          <el-slider v-model="appSettings.contentOpacity" :min="0" :max="1" :step="0.01" show-input />
        </el-form-item>
        <el-form-item label="模糊度 (Blur)">
          <el-slider v-model="appSettings.contentBlur" :min="0" :max="50" :step="1" show-input />
        </el-form-item>
      </el-form>
    </el-card>

    <br />

    <el-card>
      <template #header>
        <div class="card-header">
          <span>页面显示设置</span>
        </div>
      </template>
      <el-form label-width="140px">
        <el-form-item label="Mod管理页面">
          <el-switch v-model="appSettings.showMods" />
        </el-form-item>
        <el-form-item label="常用网址页面">
          <el-switch v-model="appSettings.showWebsites" />
        </el-form-item>
        <el-form-item label="使用文档页面">
          <el-switch v-model="appSettings.showDocuments" />
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>

<style scoped>
.settings-divider {
  display: flex;
  align-items: center;
  margin: 25px 0 15px 0;
  color: #e0e0e0;
  font-size: 14px;
  font-weight: 600;
  letter-spacing: 0.5px;
}

.settings-divider::after {
  content: '';
  flex: 1;
  height: 1px;
  background: linear-gradient(to right, rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.05));
  margin-left: 15px;
}
</style>
