<script setup lang="ts">
import { ref, onMounted } from "vue";
import { fetchOrders, type OrderDto } from "../api/contract";

const orders = ref<OrderDto[]>([]);
onMounted(async () => (orders.value = await fetchOrders()));
</script>

<template>
  <ul><li v-for="o in orders" :key="o.id">{{ o.id }}</li></ul>
</template>
