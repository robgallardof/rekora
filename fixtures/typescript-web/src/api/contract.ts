export interface OrderDto {
  id: string;
  lines: OrderLineDto[];
  rowVersion: string;
}

export interface OrderLineDto {
  sku: string;
  quantity: number;
}

export type OrderId = string;

export async function fetchOrders(): Promise<OrderDto[]> {
  const response = await fetch("/api/orders");
  return response.json();
}

export const saveOrder = async (order: OrderDto): Promise<void> => {
  await fetch(`/api/orders/${order.id}`, { method: "PUT", body: JSON.stringify(order) });
};
