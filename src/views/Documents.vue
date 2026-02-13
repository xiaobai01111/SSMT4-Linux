<template>
    <div class="documents-view">
        <iframe 
            src="https://starbobis.github.io/SSMT4-Documents/" 
            class="doc-frame"
            frameborder="0"
        ></iframe>
    </div>
</template>

<style scoped>
.documents-view {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.doc-frame {
    flex: 1;
    width: 100%;
    height: 100%;
    border: none;
    background: #fff; /* GitHub usually has white/light bg, ensure it doesn't look transparent */
}
</style>
